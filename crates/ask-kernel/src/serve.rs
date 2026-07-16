//! HTTP + WebSocket gateway + agent identity auth.
//!
//! Security model:
//!   - Tokens are opaque and resolved server-side.
//!   - The global union FOV is never broadcast.
//!   - Each web client subscribes with a list of tokens; the server sends only
//!     the union of those agents' personal FOV + memory.
//!   - No token = all-dark snapshot, zero entities.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::grid::Grid;
use crate::inspect;
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bevy_ecs::prelude::{Entity, With, World};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::actions::Action;
use crate::auth::{AgentRegistry, RegisterResult};
use crate::components::{Agent, Position, StableId};
use crate::events::EventBuf;
use crate::player::{BusPolicy, PlayerActionBus};
use crate::tick::Sim;
use crate::viewer::{build_viewer_snapshot_with, ViewerSnapshot};
use crate::vision::{self, GlowMask, VisionMap};
use crate::world::KernelWorld;

#[derive(Clone)]
struct AppState {
    sim: Arc<Mutex<Sim>>,
    /// Recent world events, capped ring (EVENT_CAP). Written by the sim
    /// thread directly — no per-tick clone of a whole Vec.
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

#[derive(Debug, Deserialize)]
struct ActionRequest {
    #[serde(default)]
    agent_id: Option<u64>,
    /// Preferred: opaque token from /api/register
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    tick: Option<u64>,
    action: Action,
}

#[derive(Debug, Serialize)]
struct ActionResponse {
    ok: bool,
    accepted: bool,
    tick: u64,
    agent_id: Option<u64>,
    human_control: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ControlRequest {
    human_control: bool,
    /// Operator credential: must be the dev token.
    #[serde(default)]
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    name: String,
    #[serde(default)]
    purpose: String,
}

#[derive(Debug, Deserialize)]
struct TrackQuery {
    token: String,
}

#[derive(Debug, Deserialize)]
struct MeQuery {
    token: String,
}

#[derive(Debug, Deserialize)]
struct SnapshotQuery {
    /// Comma-separated tracked tokens (`?token=ask1_…,ask1_…`).
    #[serde(default)]
    token: String,
    /// Optional token to use as interaction focus.
    #[serde(default)]
    focus: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EntityQuery {
    id: u64,
    /// Comma-separated tracked tokens; visibility is checked against their FOV.
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
struct CellQuery {
    x: i32,
    y: i32,
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
struct MessageSendRequest {
    token: String,
    targets: Vec<u64>,
    text: String,
}

#[derive(Debug, Serialize)]
struct MessageSendResponse {
    ok: bool,
    sent: usize,
    rejected: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn validate_action(a: &Action) -> Result<(), &'static str> {
    match a {
        Action::Move { dx, dy } => crate::actions::check_step(*dx, *dy, false),
        Action::Interact { dx, dy, .. } => crate::actions::check_step(*dx, *dy, true),
        _ => Ok(()),
    }
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

    let sim_thread = sim.clone();
    let recent_thread = recent_events.clone();
    let reg_thread = reg.clone();
    let save_thread = save_path.clone();
    std::thread::spawn(move || {
        loop {
            // Process pending agent registrations (spawn into world)
            for pending in reg_thread.drain_spawns() {
                let name = pending.name.clone();
                let purpose = pending.purpose.clone();
                let mut sim = sim_thread.blocking_lock();
                let res = match sim.kernel.spawn_agent(name.clone(), purpose.clone()) {
                    Some((id, x, y)) => {
                        let token =
                            reg_thread.bind_spawned(name.clone(), purpose.clone(), id, x, y);
                        RegisterResult {
                            ok: true,
                            token,
                            agent_id: id,
                            name,
                            purpose,
                            x,
                            y,
                            reason: None,
                        }
                    }
                    None => RegisterResult {
                        ok: false,
                        token: String::new(),
                        agent_id: 0,
                        name,
                        purpose,
                        x: 0,
                        y: 0,
                        reason: Some("no_spawn_cell".into()),
                    },
                };
                *pending.result.lock().expect("reg result") = Some(res);
                drop(sim);
            }

            {
                let mut sim = sim_thread.blocking_lock();
                sim.step();

                // Sync registry poses from world
                {
                    let mut q = sim.kernel.world.query::<(&StableId, &Position, &Agent)>();
                    let poses: Vec<_> = q
                        .iter(&mut sim.kernel.world)
                        .map(|(id, p, _)| (id.0, p.x, p.y))
                        .collect();
                    for (id, x, y) in poses {
                        reg_thread.update_pose(id, x, y, true);
                    }
                }

                let ev = sim.kernel.world.resource_mut::<EventBuf>().drain();
                {
                    let mut ring = recent_thread.blocking_lock();
                    for e in ev {
                        if ring.len() == EVENT_CAP {
                            ring.pop_front();
                        }
                        ring.push_back(e);
                    }
                }

                // periodic autosave (every 100 ticks) when --save is given
                if let Some(path) = &save_thread {
                    if sim.kernel.tick() % 100 == 0 {
                        if let Err(e) = crate::persist::save_to_path(&mut sim.kernel.world, path) {
                            eprintln!("[ask] autosave failed: {e:#}");
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(tick_ms));
        }
    });

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
    // Legacy aliases keep old clients working: /api/me, /api/action, /api/actions
    let app = Router::new()
        // --- agent core ---
        .route("/api/register", post(api_register))
        .route("/api/view", get(api_me))
        .route("/api/act", post(api_action))
        .route("/api/catalog", get(api_actions))
        .route("/api/message", post(api_message_send))
        // --- legacy aliases ---
        .route("/api/me", get(api_me))
        .route("/api/action", post(api_action))
        .route("/api/actions", get(api_actions))
        // --- spectator / web ---
        .route("/api/status", get(api_status))
        .route("/api/snapshot", get(api_snapshot))
        .route("/api/agents", get(api_agents))
        .route("/api/track", get(api_track))
        .route("/api/entity", get(api_entity))
        .route("/api/cell", get(api_cell))
        .route("/api/art", get(api_art))
        .route("/api/control", post(api_control))
        .route("/ws", get(ws_handler))
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
    eprintln!("[ask] legacy GET /api/me  POST /api/action  GET /api/actions");
    eprintln!("[ask] tick {tick_ms}ms");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn api_status(State(st): State<AppState>) -> impl IntoResponse {
    let mut sim = st.sim.lock().await;
    let (width, height) = {
        let g = sim.kernel.world.resource::<Grid>();
        (g.width, g.height)
    };
    let agents = {
        let mut q = sim.kernel.world.query_filtered::<Entity, With<Agent>>();
        q.iter(&sim.kernel.world).count()
    };
    let focused = sim
        .kernel
        .agent_entity()
        .and_then(|e| sim.kernel.world.get::<StableId>(e))
        .map(|s| s.0);
    Json(serde_json::json!({
        "ok": true,
        "tick": sim.kernel.tick(),
        "width": width,
        "height": height,
        "human_control": st.bus.human_control(),
        "pending": st.bus.pending_count(),
        "agents": agents,
        "registered": st.reg.count(),
        "agent_id": focused,
    }))
}

async fn api_snapshot(
    State(st): State<AppState>,
    Query(q): Query<SnapshotQuery>,
) -> impl IntoResponse {
    let tokens: Vec<String> = if q.token.is_empty() {
        Vec::new()
    } else {
        q.token.split(',').map(String::from).collect()
    };
    let mut sim = st.sim.lock().await;
    let recent = recent_snapshot(&st).await;
    let snap = build_snapshot_for_tokens(
        &mut sim.kernel.world,
        &recent,
        &st.reg,
        &tokens,
        q.focus.as_deref(),
    );
    Json(snap)
}

async fn api_actions() -> impl IntoResponse {
    // GET /api/catalog (alias: /api/actions) — cold reference data, not per-tick.
    Json(serde_json::json!({
        "ok": true,
        "actions": Action::catalog(),
        // generated from the verb registry — never hand-maintained
        "verbs": crate::systems::verbs::catalog_verbs().iter().map(|(v, _)| v).collect::<Vec<_>>(),
        "verb_docs": crate::systems::verbs::catalog_verbs().iter().map(|(v, d)| (v, d)).collect::<std::collections::HashMap<_,_>>(),
        "recipes": ask_kernel_recipes(),
        "api": {
            "register": "POST /api/register {name, purpose?} → {token, agent_id, x, y}",
            "view": "GET /api/view?token= → {self, view, can, inbox, events}",
            "act": "POST /api/act {token, action} → {ok, accepted, tick, reason?}",
            "catalog": "GET /api/catalog → actions/verbs/recipes (cache once)",
            "message": "POST /api/message {token, targets[], text} → inbox via view",
            "loop": "register once → view → act → view …",
            "legacy": {
                "me": "/api/me ≡ /api/view",
                "action": "/api/action ≡ /api/act",
                "actions": "/api/actions ≡ /api/catalog",
            }
        },
    }))
}

fn ask_kernel_recipes() -> Vec<serde_json::Value> {
    crate::sandbox::recipes()
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "label": r.label(),
            })
        })
        .collect()
}

async fn api_me(State(st): State<AppState>, Query(q): Query<MeQuery>) -> impl IntoResponse {
    // token → agent identity → world entity
    let Some(agent_id) = st.reg.resolve_token(&q.token) else {
        return Json(serde_json::json!({ "ok": false, "reason": "invalid_token" }));
    };
    let mut sim = st.sim.lock().await;
    let agent_e = {
        let mut q2 = sim.kernel.world.query::<(Entity, &StableId)>();
        q2.iter(&sim.kernel.world)
            .find(|(_, sid)| sid.0 == agent_id)
            .map(|(e, _)| e)
    };
    let Some(agent_e) = agent_e else {
        return Json(serde_json::json!({ "ok": false, "reason": "no_agent" }));
    };
    let recent = recent_snapshot(&st).await;
    let Some(view) = crate::agent_view::build_agent_view(&mut sim.kernel.world, agent_e, &recent)
    else {
        return Json(serde_json::json!({ "ok": false, "reason": "no_agent" }));
    };

    // Canonical shape: self / view / can / inbox / events.
    // Flat aliases below keep older clients working (slated for removal).
    let mut v = view;
    {
        let obj = v.as_object_mut().expect("view is object");
        let selfb = obj["self"].clone();
        let can = obj["can"].clone();
        for k in [
            "id", "name", "x", "y", "hp", "max_hp", "wood", "iron", "items", "pack",
        ] {
            obj.insert(k.into(), selfb[k].clone());
        }
        obj.insert("underfoot".into(), can["underfoot"].clone());
        obj.insert("here".into(), can["here"].clone());
        obj.insert("adjacent".into(), can["adjacent"].clone());
        obj.insert("interactions".into(), can["interactions"].clone());
        let events = obj["events"].clone();
        obj.insert("recent_events".into(), events);
        let inbox = obj["inbox"].clone();
        obj.insert("messages".into(), inbox);
    }
    Json(v)
}

async fn api_register(
    State(st): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 32 {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "name required (1-32 chars)",
        }));
    }
    let purpose = req.purpose.trim().chars().take(120).collect::<String>();
    let handle = st.reg.request_register(name, purpose);

    // Wait up to ~2s for sim thread to process spawn
    for _ in 0..40 {
        if let Some(res) = handle.lock().expect("reg").clone() {
            return Json(serde_json::json!(res));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Json(serde_json::json!({
        "ok": false,
        "reason": "spawn_timeout",
    }))
}

async fn api_agents(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "agents": st.reg.list_public(),
    }))
}

async fn api_track(State(st): State<AppState>, Query(q): Query<TrackQuery>) -> impl IntoResponse {
    match st.reg.public_for_token(&q.token) {
        Some(p) => Json(serde_json::json!({
            "ok": true,
            "agent_id": p.agent_id,
            "name": p.name,
            "purpose": p.purpose,
            "x": p.x,
            "y": p.y,
            "alive": p.alive,
            "token_prefix": q.token.chars().take(12).collect::<String>(),
        })),
        None => Json(serde_json::json!({
            "ok": false,
            "reason": "unknown_token",
        })),
    }
}

/// Compute the visibility map a player is allowed to use for inspect queries.
/// Returns `None` when the supplied tokens are empty/unknown (no permission).
fn player_visible_map(
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

async fn api_art() -> impl IntoResponse {
    let c = crate::art::catalog();
    Json(serde_json::json!({
        "ok": true,
        "catalog_version": c.catalog_version,
        "materials": c.materials,
        "feats": c.feats,
        "races": c.races,
        "objects": c.objects,
        "entity_defaults": c.entity_defaults,
    }))
}

async fn api_entity(
    State(st): State<AppState>,
    Query(q): Query<EntityQuery>,
) -> impl IntoResponse {
    let mut sim = st.sim.lock().await;
    let Some(vis) = player_visible_map(&mut sim.kernel.world, &st.reg, &q.token) else {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "unauthorized",
        }));
    };

    let Some(info) = inspect::entity_info(&mut sim.kernel.world, q.id) else {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "not_found",
        }));
    };

    let is_dev = q
        .token
        .split(',')
        .map(str::trim)
        .any(|t| st.reg.is_dev_token(t));
    if !is_dev {
        let x = info["x"].as_i64().unwrap_or(0) as i32;
        let y = info["y"].as_i64().unwrap_or(0) as i32;
        if !vis.is_visible(x, y) {
            return Json(serde_json::json!({
                "ok": false,
                "reason": "not_visible",
            }));
        }
    }

    Json(serde_json::json!({ "ok": true, "entity": info }))
}

async fn api_cell(State(st): State<AppState>, Query(q): Query<CellQuery>) -> impl IntoResponse {
    let mut sim = st.sim.lock().await;
    let Some(vis) = player_visible_map(&mut sim.kernel.world, &st.reg, &q.token) else {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "unauthorized",
        }));
    };

    if vis.display_class(q.x, q.y) == 0 {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "not_visible",
        }));
    }

    match inspect::cell_info(&mut sim.kernel.world, q.x, q.y) {
        Some(info) => Json(serde_json::json!({ "ok": true, "cell": info })),
        None => Json(serde_json::json!({
            "ok": false,
            "reason": "out_of_bounds",
        })),
    }
}

async fn api_message_send(
    State(st): State<AppState>,
    Json(req): Json<MessageSendRequest>,
) -> impl IntoResponse {
    const MAX_LEN: usize = 500;
    let text = req.text.trim().to_string();
    if text.is_empty() {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("empty text".into()),
        });
    }
    if text.encode_utf16().count() > MAX_LEN {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("text too long".into()),
        });
    }

    let mut sim = st.sim.lock().await;
    let Some(vis) = player_visible_map(&mut sim.kernel.world, &st.reg, &req.token) else {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("unauthorized".into()),
        });
    };

    let is_dev = req
        .token
        .split(',')
        .map(str::trim)
        .any(|t| st.reg.is_dev_token(t));
    // Sender identity comes from the token (agent name), never the wire IP —
    // recipients need to know *who* in the world wrote to them.
    let sender_name = if is_dev {
        "DEV".to_string()
    } else {
        req.token
            .split(',')
            .map(str::trim)
            .find_map(|t| st.reg.public_for_token(t))
            .map(|p| p.name)
            .unwrap_or_else(|| "anon".into())
    };
    let tick = sim.kernel.tick();

    // Build a StableId -> (Entity, Position) map once.
    let agents: std::collections::HashMap<u64, (Entity, i32, i32)> = {
        let mut q = sim
            .kernel
            .world
            .query::<(Entity, &StableId, &Position, &Agent)>();
        q.iter(&sim.kernel.world)
            .map(|(e, sid, p, _)| (sid.0, (e, p.x, p.y)))
            .collect()
    };

    let mut sent = 0;
    let mut rejected = 0;

    for target_id in req.targets {
        let Some((entity, x, y)) = agents.get(&target_id).copied() else {
            rejected += 1;
            continue;
        };

        if !is_dev && !vis.is_visible(x, y) {
            rejected += 1;
            continue;
        }

        let id = {
            let mut counter = sim.kernel.world.resource_mut::<crate::components::MessageCounter>();
            let id = counter.0;
            counter.0 += 1;
            id
        };

        let Some(mut mailbox) = sim.kernel.world.get_mut::<crate::components::AgentMailbox>(entity)
        else {
            rejected += 1;
            continue;
        };

        mailbox.push(crate::components::Envelope {
            id,
            from: sender_name.clone(),
            text: text.clone(),
            sent_tick: tick,
            read: false,
        });
        sent += 1;
    }

    Json(MessageSendResponse {
        ok: true,
        sent,
        rejected,
        reason: None,
    })
}

async fn api_action(
    State(st): State<AppState>,
    Json(req): Json<ActionRequest>,
) -> impl IntoResponse {
    let tick = {
        let sim = st.sim.lock().await;
        sim.kernel.tick()
    };
    if let Err(reason) = validate_action(&req.action) {
        return Json(ActionResponse {
            ok: true,
            accepted: false,
            tick,
            agent_id: req.agent_id,
            human_control: st.bus.human_control(),
            reason: Some(reason.into()),
        });
    }

    // Identity: token is mandatory. A bare `agent_id` is NOT an identity —
    // it lets any client drive any agent. When both are sent they must match.
    let reject = |reason: &str, agent_id: Option<u64>| {
        Json(ActionResponse {
            ok: true,
            accepted: false,
            tick,
            agent_id,
            human_control: st.bus.human_control(),
            reason: Some(reason.into()),
        })
    };
    let Some(ref tok) = req.token else {
        return reject("token_required", None);
    };
    let Some(agent_id) = st.reg.resolve_token(tok) else {
        return reject("invalid_token", None);
    };
    if let Some(declared) = req.agent_id {
        if declared != agent_id {
            return reject("agent_id_token_mismatch", Some(agent_id));
        }
    }

    st.bus.submit(Some(agent_id), req.action, req.tick.or(Some(tick)));
    Json(ActionResponse {
        ok: true,
        accepted: true,
        tick,
        agent_id: Some(agent_id),
        human_control: st.bus.human_control(),
        reason: None,
    })
}

async fn api_control(
    State(st): State<AppState>,
    Json(req): Json<ControlRequest>,
) -> impl IntoResponse {
    // World-wide switch: operator-only (dev token), same as WS control.
    let ok_auth = req
        .token
        .as_deref()
        .map(|t| st.reg.is_dev_token(t))
        .unwrap_or(false);
    if !ok_auth {
        let tick = {
            let sim = st.sim.lock().await;
            sim.kernel.tick()
        };
        return Json(serde_json::json!({
            "ok": false,
            "reason": "dev_token_required",
            "human_control": st.bus.human_control(),
            "tick": tick,
        }));
    }
    st.bus.set_human_control(req.human_control);
    let tick = {
        let sim = st.sim.lock().await;
        sim.kernel.tick()
    };
    Json(serde_json::json!({
        "ok": true,
        "human_control": st.bus.human_control(),
        "tick": tick,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(st): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_ws(socket, st))
}

async fn client_ws(mut socket: WebSocket, st: AppState) {
    let mut tokens: Vec<String> = Vec::new();
    let mut focus_token: Option<String> = None;
    let mut interval = tokio::time::interval(Duration::from_millis(st.tick_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let json = {
                    let mut sim = st.sim.lock().await;
                    let recent = recent_snapshot(&st).await;
                    let snap = if tokens.is_empty() {
                        let grid = sim.kernel.world.resource::<Grid>();
                        let dark = VisionMap::new(grid.width, grid.height);
                        build_viewer_snapshot_with(
                            &mut sim.kernel.world,
                            &recent,
                            &dark,
                            Some(&[]),
                            None,
                            false,
                        )
                    } else {
                        build_snapshot_for_tokens(
                            &mut sim.kernel.world,
                            &recent,
                            &st.reg,
                            &tokens,
                            focus_token.as_deref(),
                        )
                    };
                    serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into())
                };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                                "subscribe" => {
                                    let toks: Vec<String> = v.get("tokens")
                                        .and_then(|t| t.as_array())
                                        .map(|a| a.iter()
                                            .filter_map(|x| x.as_str().map(String::from))
                                            .collect())
                                        .unwrap_or_default();
                                    let focus = v.get("focus")
                                        .and_then(|x| x.as_str().map(String::from));
                                    tokens = toks;
                                    focus_token = focus;
                                }
                                "action" => handle_ws_action(&st, &v).await,
                                "control" => handle_ws_control(&st, &v),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn handle_ws_action(st: &AppState, v: &serde_json::Value) {
    // Same rule as POST /api/act: token is the only identity.
    let Some(token) = v.get("token").and_then(|x| x.as_str()) else {
        return;
    };
    let Some(agent_id) = st.reg.resolve_token(token) else {
        return;
    };
    if let Some(declared) = v.get("agent_id").and_then(|x| x.as_u64()) {
        if declared != agent_id {
            return;
        }
    }
    let tick = {
        let sim = st.sim.lock().await;
        sim.kernel.tick()
    };
    let Some(av) = v.get("action") else { return };
    let Ok(action) = serde_json::from_value::<Action>(av.clone()) else {
        return;
    };
    if validate_action(&action).is_err() {
        return;
    }
    st.bus.submit(Some(agent_id), action, Some(tick));
}

fn handle_ws_control(st: &AppState, v: &serde_json::Value) {
    // World-wide switches are operator-only: dev token required.
    let token = v.get("token").and_then(|x| x.as_str()).unwrap_or("");
    if !st.reg.is_dev_token(token) {
        return;
    }
    if let Some(hc) = v.get("human_control").and_then(|x| x.as_bool()) {
        st.bus.set_human_control(hc);
    }
}

/// Build a player-specific snapshot for a set of tracked tokens.
fn build_snapshot_for_tokens(
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
        return build_viewer_snapshot_with(world, recent_events, &vis, None, None, true);
    }

    let ids: Vec<u64> = tokens.iter().filter_map(|t| reg.resolve_token(t)).collect();
    if ids.is_empty() {
        let grid = world.resource::<Grid>();
        let dark = VisionMap::new(grid.width, grid.height);
        return build_viewer_snapshot_with(world, recent_events, &dark, Some(&[]), None, false);
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
    build_viewer_snapshot_with(world, recent_events, &vis, Some(&ids), focus_id, false)
}
