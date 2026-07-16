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
use bevy_ecs::prelude::{Entity, World};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::actions::Action;
use crate::auth::{AgentRegistry, RegisterResult};
use crate::components::{Agent, Position, StableId};
use crate::config::Config;
use crate::events::EventBuf;
use crate::player::{BusPolicy, PlayerActionBus};
use crate::tick::Sim;
use crate::viewer::{build_viewer_snapshot, build_viewer_snapshot_with, ViewerSnapshot};
use crate::vision::{self, GlowMask, VisionMap};
use crate::world::KernelWorld;

#[derive(Clone)]
struct AppState {
    sim: Arc<Mutex<Sim>>,
    recent_events: Arc<Mutex<Vec<crate::events::GameEvent>>>,
    bus: PlayerActionBus,
    reg: AgentRegistry,
    tick_ms: u64,
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

fn validate_action(a: &Action) -> Result<(), &'static str> {
    match a {
        Action::Move { dx, dy } => {
            if dx.abs() + dy.abs() != 1 {
                Err("move needs four-way unit step")
            } else {
                Ok(())
            }
        }
        Action::Interact { dx, dy, .. } => {
            if !(*dx == 0 && *dy == 0) && dx.abs() + dy.abs() != 1 {
                Err("interact: underfoot or adjacent only")
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

pub async fn run_server(port: u16, tick_ms: u64, cfg: Config) -> Result<()> {
    let kernel = KernelWorld::new(&cfg);
    let bus = PlayerActionBus::new();
    let reg = AgentRegistry::new(cfg.seed);
    let sim = Arc::new(Mutex::new(Sim::with_policy(
        kernel,
        Box::new(BusPolicy::new(bus.clone(), true)),
    )));
    let recent_events = Arc::new(Mutex::new(Vec::new()));

    let sim_thread = sim.clone();
    let recent_thread = recent_events.clone();
    let reg_thread = reg.clone();
    std::thread::spawn(move || {
        let mut recent: Vec<crate::events::GameEvent> = Vec::new();
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

                let mut ev = sim.kernel.world.resource_mut::<EventBuf>().drain();
                recent.extend(ev.drain(..));
                if recent.len() > 40 {
                    let n = recent.len() - 40;
                    recent.drain(0..n);
                }
            }
            *recent_thread.blocking_lock() = recent.clone();

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

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/snapshot", get(api_snapshot))
        .route("/api/actions", get(api_actions))
        .route("/api/me", get(api_me))
        .route("/api/action", post(api_action))
        .route("/api/control", post(api_control))
        .route("/api/register", post(api_register))
        .route("/api/agents", get(api_agents))
        .route("/api/track", get(api_track))
        .route("/api/entity", get(api_entity))
        .route("/api/cell", get(api_cell))
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
    eprintln!("[ask] GET  /api/snapshot /api/me /api/actions /api/agents /api/track");
    eprintln!("[ask] POST /api/register /api/action /api/control");
    eprintln!("[ask] WS   /ws");
    eprintln!("[ask] tick {tick_ms}ms");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_status(State(st): State<AppState>) -> impl IntoResponse {
    let mut sim = st.sim.lock().await;
    let snap = build_viewer_snapshot(&mut sim.kernel.world, &[]);
    Json(serde_json::json!({
        "ok": true,
        "tick": sim.kernel.tick(),
        "width": snap.width,
        "height": snap.height,
        "human_control": st.bus.human_control(),
        "pending": st.bus.pending_count(),
        "agents": snap.entities.iter().filter(|e| e.kind == "agent").count(),
        "registered": st.reg.count(),
        "agent_id": snap.focused_agent_id,
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
    let recent = st.recent_events.lock().await;
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
    Json(serde_json::json!({
        "ok": true,
        "actions": Action::catalog(),
        "verbs": [
            "dig","scoop","place","harvest","plant","build","deconstruct","craft",
            "pickup","open","close","descend","ascend","attack"
        ],
        "recipes": ask_kernel_recipes(),
        "auth": {
            "register": "POST /api/register {name, purpose} → {token, agent_id}",
            "act": "POST /api/action {token, action}",
            "track": "GET /api/track?token=ask1_…",
        },
        "loop": "register → me(token) → interact → events",
    }))
}

fn ask_kernel_recipes() -> Vec<serde_json::Value> {
    crate::sandbox::recipes()
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "label": r.label,
            })
        })
        .collect()
}

async fn api_me(State(st): State<AppState>, Query(q): Query<MeQuery>) -> impl IntoResponse {
    let tok = q.token.as_str();
    let mut sim = st.sim.lock().await;
    let recent = st.recent_events.lock().await;
    let snap = build_snapshot_for_tokens(
        &mut sim.kernel.world,
        &recent,
        &st.reg,
        &[tok.to_string()],
        Some(tok),
    );

    let Some(a) = snap.entities.iter().find(|e| e.kind == "agent") else {
        return Json(serde_json::json!({ "ok": false, "reason": "no_agent" }));
    };
    let (x, y) = (a.x, a.y);
    let underfoot = snap
        .tiles
        .get(y as usize)
        .and_then(|row| row.chars().nth(x as usize))
        .unwrap_or(' ');
    let vis = snap
        .vision
        .get(y as usize)
        .and_then(|row| row.chars().nth(x as usize))
        .unwrap_or(' ');
    let here: Vec<_> = snap
        .entities
        .iter()
        .filter(|e| e.x == x && e.y == y && e.kind != "agent")
        .map(|e| {
            serde_json::json!({
                "id": e.id, "kind": e.kind, "glyph": e.glyph,
                "name": e.name, "amount": e.amount,
            })
        })
        .collect();
    let adj: Vec<_> = snap
        .entities
        .iter()
        .filter(|e| e.kind != "agent" && (e.x - x).abs() + (e.y - y).abs() == 1)
        .map(|e| {
            serde_json::json!({
                "id": e.id, "kind": e.kind, "x": e.x, "y": e.y,
                "dx": e.x - x, "dy": e.y - y,
                "glyph": e.glyph, "name": e.name,
            })
        })
        .collect();

    Json(serde_json::json!({
        "ok": true,
        "tick": snap.tick,
        "id": a.id,
        "name": a.name,
        "x": x, "y": y,
        "hp": a.hp, "max_hp": a.max_hp,
        "wood": a.wood, "iron": a.iron,
        "items": a.items,
        "pack": a.pack,
        "underfoot": { "glyph": underfoot, "vision": vis },
        "here": here,
        "adjacent": adj,
        "interactions": snap.interactions,
        "recent_events": snap.recent_events,
    }))
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

    // Resolve identity: token preferred
    let agent_id = if let Some(ref tok) = req.token {
        match st.reg.resolve_token(tok) {
            Some(id) => Some(id),
            None => {
                return Json(ActionResponse {
                    ok: true,
                    accepted: false,
                    tick,
                    agent_id: None,
                    human_control: st.bus.human_control(),
                    reason: Some("invalid_token".into()),
                });
            }
        }
    } else {
        req.agent_id
    };

    st.bus.submit(agent_id, req.action, req.tick.or(Some(tick)));
    Json(ActionResponse {
        ok: true,
        accepted: true,
        tick,
        agent_id,
        human_control: st.bus.human_control(),
        reason: None,
    })
}

async fn api_control(
    State(st): State<AppState>,
    Json(req): Json<ControlRequest>,
) -> impl IntoResponse {
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
                    let recent = st.recent_events.lock().await;
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
    let token = v.get("token").and_then(|x| x.as_str());
    let agent_id = if let Some(tok) = token {
        st.reg.resolve_token(tok)
    } else {
        v.get("agent_id").and_then(|x| x.as_u64())
    };
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
    st.bus.submit(agent_id, action, Some(tick));
}

fn handle_ws_control(st: &AppState, v: &serde_json::Value) {
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
