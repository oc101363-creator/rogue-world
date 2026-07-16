//! HTTP + WebSocket gateway — router, shared state, sim driver.
//!
//! Security model:
//!   - Tokens are opaque and resolved server-side.
//!   - The global union FOV is never broadcast.
//!   - Each web client subscribes with a list of tokens; the server sends only
//!     the union of those agents' personal FOV + memory.
//!   - No token = all-dark snapshot, zero entities.
//!
//! Layout: `api.rs` = HTTP handlers, `ws.rs` = the websocket loop,
//! this file = AppState + sim task + shared visibility helpers.

mod api;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use bevy_ecs::prelude::{Entity, World};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::auth::{AgentRegistry, RegisterResult};
use crate::components::{Agent, Position, StableId};
use crate::events::EventBuf;
use crate::grid::Grid;
use crate::player::{BusPolicy, PlayerActionBus};
use crate::tick::Sim;
use crate::viewer::{build_viewer_snapshot_with, ViewerSnapshot};
use crate::vision::{self, GlowMask, VisionMap};
use crate::world::KernelWorld;

#[derive(Clone)]
pub(crate) struct AppState {
    sim: Arc<Mutex<Sim>>,
    /// Recent world events, capped ring (EVENT_CAP). Written by the sim
    /// task directly — no per-tick clone of a whole Vec.
    recent_events: Arc<Mutex<std::collections::VecDeque<crate::events::GameEvent>>>,
    bus: PlayerActionBus,
    reg: AgentRegistry,
    tick_ms: u64,
}

const EVENT_CAP: usize = 40;

/// Copy the ring out for a request (40 items, trivial).
async fn recent_snapshot(st: &AppState) -> Vec<crate::events::GameEvent> {
    st.recent_events.lock().await.iter().cloned().collect()
}

pub async fn run_server(
    port: u16,
    tick_ms: u64,
    kernel: KernelWorld,
    save_path: Option<String>,
) -> Result<()> {
    let seed = kernel
        .world
        .get_resource::<crate::world::WorldSeed>()
        .map(|s| s.0)
        .unwrap_or(1);
    let bus = PlayerActionBus::new();
    let reg = AgentRegistry::new(seed);
    let sim = Arc::new(Mutex::new(Sim::with_policy(
        kernel,
        Box::new(BusPolicy::new(bus.clone(), true)),
    )));
    let recent_events = Arc::new(Mutex::new(std::collections::VecDeque::new()));

    // Sim driver: a plain tokio task (no thread + blocking_lock hybrids).
    {
        let sim = sim.clone();
        let recent = recent_events.clone();
        let reg = reg.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;

                // registrations → spawn into world, resolve one-shots
                for pending in reg.drain_spawns() {
                    let mut sim = sim.lock().await;
                    let res = match sim
                        .kernel
                        .spawn_agent(pending.name.clone(), pending.purpose.clone())
                    {
                        Some((id, x, y)) => {
                            let token =
                                reg.bind_spawned(pending.name.clone(), pending.purpose.clone(), id, x, y);
                            RegisterResult {
                                ok: true,
                                token,
                                agent_id: id,
                                name: pending.name,
                                purpose: pending.purpose,
                                x,
                                y,
                                reason: None,
                            }
                        }
                        None => RegisterResult {
                            ok: false,
                            token: String::new(),
                            agent_id: 0,
                            name: pending.name,
                            purpose: pending.purpose,
                            x: 0,
                            y: 0,
                            reason: Some("no_spawn_cell".into()),
                        },
                    };
                    let _ = pending.result.send(res);
                    drop(sim);
                }

                {
                    let mut sim = sim.lock().await;
                    sim.step();

                    // Sync registry poses from world
                    {
                        let mut q = sim.kernel.world.query::<(&StableId, &Position, &Agent)>();
                        let poses: Vec<_> = q
                            .iter(&mut sim.kernel.world)
                            .map(|(id, p, _)| (id.0, p.x, p.y))
                            .collect();
                        for (id, x, y) in poses {
                            reg.update_pose(id, x, y, true);
                        }
                    }

                    let ev = sim.kernel.world.resource_mut::<EventBuf>().drain();
                    {
                        let mut ring = recent.lock().await;
                        for e in ev {
                            if ring.len() == EVENT_CAP {
                                ring.pop_front();
                            }
                            ring.push_back(e);
                        }
                    }

                    // periodic autosave (every 100 ticks) when --save is given
                    if let Some(path) = &save_path {
                        if sim.kernel.tick() % 100 == 0 {
                            if let Err(e) =
                                crate::persist::save_to_path(&mut sim.kernel.world, path)
                            {
                                eprintln!("[ask] autosave failed: {e:#}");
                            }
                        }
                    }
                }
            }
        });
    }

    eprintln!("[ask] dev token: {}", reg.dev_token());

    let state = AppState {
        sim,
        recent_events,
        bus,
        reg,
        tick_ms,
    };
    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let index = static_dir.join("index.html");

    // API surface (layered):
    //   Agent loop:   POST /api/register · GET /api/view · POST /api/act · GET /api/catalog
    //   Social:       POST /api/message  (inbox appears in view)
    //   Spectator:    GET /api/snapshot · /api/track · /api/agents · /api/entity · /api/cell · WS /ws
    //   Presentation: GET /api/art
    //   Ops:          GET /api/status · POST /api/control
    let app = Router::new()
        // --- agent core ---
        .route("/api/register", post(api::register))
        .route("/api/view", get(api::view))
        .route("/api/act", post(api::act))
        .route("/api/catalog", get(api::catalog))
        .route("/api/message", post(api::message_send))
        // --- spectator / web ---
        .route("/api/status", get(api::status))
        .route("/api/snapshot", get(api::snapshot))
        .route("/api/agents", get(api::agents))
        .route("/api/track", get(api::track))
        .route("/api/entity", get(api::entity))
        .route("/api/cell", get(api::cell))
        .route("/api/art", get(api::art))
        .route("/api/control", post(api::control))
        .route("/ws", get(ws::handler))
        .route_service("/", ServeFile::new(index))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    eprintln!("[ask] http://{addr}/");
    eprintln!("[ask] agent  POST /api/register  GET /api/view  POST /api/act  GET /api/catalog");
    eprintln!("[ask] social POST /api/message");
    eprintln!("[ask] spect  GET /api/snapshot /api/track /api/agents /api/entity /api/cell /api/art");
    eprintln!("[ask] ops    GET /api/status  POST /api/control  WS /ws");
    eprintln!("[ask] tick {tick_ms}ms");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

/// Compute the visibility map a player is allowed to use for inspect queries.
/// Returns `None` when the supplied tokens are empty/unknown (no permission).
pub(crate) fn player_visible_map(
    world: &mut World,
    reg: &AgentRegistry,
    tokens: &str,
) -> Option<VisionMap> {
    let toks: Vec<&str> = if tokens.is_empty() {
        Vec::new()
    } else {
        tokens.split(',').map(str::trim).collect()
    };

    if toks.iter().any(|t| reg.is_dev_token(t)) {
        let grid = world.resource::<Grid>();
        let mut vis = VisionMap::new(grid.width, grid.height);
        for f in vis.flags.iter_mut() {
            *f = vision::F_VIEW | vision::F_LITE | vision::F_GLOW | vision::F_MARK;
        }
        return Some(vis);
    }

    let ids: Vec<u64> = toks.iter().filter_map(|t| reg.resolve_token(t)).collect();
    if ids.is_empty() {
        return None;
    }

    let mut agent_entities = Vec::new();
    {
        let mut q = world.query::<(Entity, &StableId, &Agent)>();
        for (e, sid, _) in q.iter(world) {
            if ids.contains(&sid.0) {
                agent_entities.push(e);
            }
        }
    }

    Some(vision::compute_view_for_agents(world, &agent_entities))
}

/// Build a player-specific snapshot for a set of tracked tokens.
pub(crate) fn build_snapshot_for_tokens(
    world: &mut World,
    recent_events: &[crate::events::GameEvent],
    reg: &AgentRegistry,
    tokens: &[String],
    focus_token: Option<&str>,
) -> ViewerSnapshot {
    // Developer omniscient token: full map + all agents.
    if tokens.iter().any(|t| reg.is_dev_token(t)) {
        let grid = world.resource::<Grid>();
        let glow = world
            .get_resource::<GlowMask>()
            .cloned()
            .unwrap_or_else(|| GlowMask::new(grid.width, grid.height));
        let mut vis = VisionMap::from_glow(grid.width, grid.height, &glow);
        for f in vis.flags.iter_mut() {
            *f |= vision::F_VIEW | vision::F_LITE;
        }
        return build_viewer_snapshot_with(world, recent_events, &vis, None, None);
    }

    let ids: Vec<u64> = tokens.iter().filter_map(|t| reg.resolve_token(t)).collect();
    if ids.is_empty() {
        let grid = world.resource::<Grid>();
        let dark = VisionMap::new(grid.width, grid.height);
        return build_viewer_snapshot_with(world, recent_events, &dark, Some(&[]), None);
    }

    let mut agent_entities = Vec::new();
    {
        let mut q = world.query::<(Entity, &StableId, &Agent)>();
        for (e, sid, _) in q.iter(world) {
            if ids.contains(&sid.0) {
                agent_entities.push(e);
            }
        }
    }

    let vis = vision::compute_view_for_agents(world, &agent_entities);
    let focus_id = focus_token
        .and_then(|t| reg.resolve_token(t))
        .or(ids.first().copied());
    build_viewer_snapshot_with(world, recent_events, &vis, Some(&ids), focus_id)
}
