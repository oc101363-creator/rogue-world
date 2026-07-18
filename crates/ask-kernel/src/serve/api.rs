//! HTTP handlers — one thin function per endpoint. Handlers never reach
//! into gameplay internals; they resolve identity, call a projection
//! (agent_view / viewer / inspect), and serialize.

use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use bevy_ecs::prelude::{Entity, With};
use serde::{Deserialize, Serialize};

use std::net::SocketAddr;
use std::time::Duration;

use super::{build_snapshot_for_tokens, player_visible_map, recent_snapshot, AppState};
use crate::actions::Action;
use crate::components::{Agent, StableId};
use crate::grid::Grid;
use crate::inspect;

#[derive(Debug, Deserialize)]
pub(crate) struct ActionRequest {
    #[serde(default)]
    agent_id: Option<u64>,
    /// Opaque token from /api/register — the only accepted identity.
    #[serde(default)]
    token: Option<String>,
    /// "I decided from the view at this tick" — server answers with
    /// `ticks_behind` so the agent knows how stale its worldview was.
    #[serde(default)]
    base_tick: Option<u64>,
    /// Client-chosen monotonic idempotency key (per agent). A duplicate or
    /// older seq is rejected — network retries can't double-apply an act.
    #[serde(default)]
    seq: Option<u64>,
    action: Action,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActionResponse {
    ok: bool,
    accepted: bool,
    /// Tick at submit time.
    tick: u64,
    /// The tick the action LANDS on (tick+1, exact: submit happens under
    /// the sim lock, so no tick can slip in between).
    applied_tick: u64,
    /// True when this submit overwrote a still-pending earlier action
    /// (last-write-wins within one tick).
    replaced: bool,
    /// Present when the request carried base_tick: how many ticks the
    /// world had moved past the view the agent decided from.
    #[serde(skip_serializing_if = "Option::is_none")]
    ticks_behind: Option<u64>,
    agent_id: Option<u64>,
    human_control: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ControlRequest {
    human_control: bool,
    /// Operator credential: must be the dev token.
    #[serde(default)]
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterRequest {
    name: String,
    #[serde(default)]
    purpose: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenQuery {
    token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ViewQuery {
    token: String,
    /// Long-poll: hold the request until world tick > after_tick.
    /// The natural pair to act's `applied_tick` — one round trip from
    /// "I acted" to "here is what my act did".
    #[serde(default)]
    after_tick: Option<u64>,
    /// Long-poll hold cap in ms (default 10s, hard max 30s).
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SnapshotQuery {
    /// Comma-separated tracked tokens (`?token=ask1_…,ask1_…`).
    #[serde(default)]
    token: String,
    /// Optional token to use as interaction focus.
    #[serde(default)]
    focus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntityQuery {
    id: u64,
    /// Comma-separated tracked tokens; visibility is checked against their FOV.
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CellQuery {
    x: i32,
    y: i32,
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MessageSendRequest {
    token: String,
    targets: Vec<u64>,
    text: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MessageSendResponse {
    ok: bool,
    sent: usize,
    rejected: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// Pre-submit shape check (shared with the WS action path).
pub(crate) fn validate_action(a: &Action) -> Result<(), &'static str> {
    match a {
        Action::Move { dx, dy } => crate::actions::check_step(*dx, *dy, false),
        Action::Interact { dx, dy, .. } => crate::actions::check_step(*dx, *dy, true),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------- agent core

pub(crate) async fn register(
    State(st): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Spawn-per-registration is the main abuse vector: throttle by IP.
    if !st
        .rate
        .check(&format!("register:{}", addr.ip()), 10, Duration::from_secs(60))
    {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "rate_limited",
        }));
    }
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 32 {
        return Json(serde_json::json!({
            "ok": false,
            "reason": "name required (1-32 chars)",
        }));
    }
    let purpose = req.purpose.trim().chars().take(120).collect::<String>();
    let rx = st.reg.request_register(name, purpose);

    // Resolved by the sim task via one-shot (no polling loop).
    match tokio::time::timeout(std::time::Duration::from_secs(2), rx).await {
        Ok(Ok(res)) => Json(serde_json::json!(res)),
        _ => Json(serde_json::json!({
            "ok": false,
            "reason": "spawn_timeout",
        })),
    }
}

pub(crate) async fn view(
    State(st): State<AppState>,
    Query(q): Query<ViewQuery>,
) -> impl IntoResponse {
    // token → agent identity → world entity
    let Some(agent_id) = st.reg.resolve_token(&q.token) else {
        return Json(serde_json::json!({ "ok": false, "reason": "invalid_token" }));
    };
    // Long-poll BEFORE touching the sim lock: wait for the world to move
    // past after_tick (or the hold cap), then read a fresh view.
    if let Some(after) = q.after_tick {
        let cap = Duration::from_millis(q.timeout_ms.unwrap_or(10_000).min(30_000));
        let mut rx = st.tick_rx.clone();
        let wait = async move {
            loop {
                if *rx.borrow_and_update() > after {
                    break;
                }
                if rx.changed().await.is_err() {
                    break; // sim gone — fall through to a normal view
                }
            }
        };
        let _ = tokio::time::timeout(cap, wait).await;
    }
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
    match crate::agent_view::build_agent_view(&mut sim.kernel.world, agent_e) {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "ok": false, "reason": "no_agent" })),
    }
}

pub(crate) async fn act(
    State(st): State<AppState>,
    Json(req): Json<ActionRequest>,
) -> impl IntoResponse {
    let respond = |accepted: bool,
                   tick: u64,
                   applied_tick: u64,
                   replaced: bool,
                   ticks_behind: Option<u64>,
                   agent_id: Option<u64>,
                   human: bool,
                   reason: Option<&str>| {
        Json(ActionResponse {
            ok: true,
            accepted,
            tick,
            applied_tick,
            replaced,
            ticks_behind,
            agent_id,
            human_control: human,
            reason: reason.map(String::from),
        })
    };

    // Shape check is cheap and needs no identity.
    if let Err(reason) = validate_action(&req.action) {
        let cur = *st.tick_rx.borrow();
        return respond(
            false,
            cur,
            cur + 1,
            false,
            None,
            req.agent_id,
            st.bus.human_control(),
            Some(reason),
        );
    }

    // Identity: token is mandatory. A bare `agent_id` is NOT an identity —
    // it lets any client drive any agent. When both are sent they must match.
    let cur = *st.tick_rx.borrow();
    let Some(ref tok) = req.token else {
        return respond(false, cur, cur + 1, false, None, None, st.bus.human_control(), Some("token_required"));
    };
    let Some(agent_id) = st.reg.resolve_token(tok) else {
        return respond(false, cur, cur + 1, false, None, None, st.bus.human_control(), Some("invalid_token"));
    };
    if !st
        .rate
        .check(&format!("act:{tok}"), 40, Duration::from_secs(10))
    {
        return respond(false, cur, cur + 1, false, None, Some(agent_id), st.bus.human_control(), Some("rate_limited"));
    }
    if let Some(declared) = req.agent_id {
        if declared != agent_id {
            return respond(false, cur, cur + 1, false, None, Some(agent_id), st.bus.human_control(), Some("agent_id_token_mismatch"));
        }
    }
    // Idempotency: client seqs must strictly increase per agent.
    if let Some(seq) = req.seq {
        let mut g = st.seq.lock().expect("seq map");
        let last = g.get(&agent_id).copied().unwrap_or(0);
        if seq <= last {
            return respond(false, cur, cur + 1, false, None, Some(agent_id), st.bus.human_control(), Some("duplicate_seq"));
        }
        g.insert(agent_id, seq);
    }

    // Submit under the sim lock: no tick can slip between the read and the
    // enqueue, so the action provably lands on tick+1.
    let (tick, replaced, behind) = {
        let sim = st.sim.lock().await;
        let tick = sim.kernel.tick();
        let replaced = st.bus.submit(Some(agent_id), req.action);
        let behind = req.base_tick.map(|b| tick.saturating_sub(b));
        (tick, replaced, behind)
    };
    respond(true, tick, tick + 1, replaced, behind, Some(agent_id), st.bus.human_control(), None)
}

pub(crate) async fn catalog() -> impl IntoResponse {
    // GET /api/catalog — cold reference data, not per-tick.
    Json(serde_json::json!({
        "ok": true,
        "actions": Action::catalog(),
        // generated from the verb registry — never hand-maintained
        "verbs": crate::systems::verbs::catalog_verbs().iter().map(|(v, _)| v).collect::<Vec<_>>(),
        "verb_docs": crate::systems::verbs::catalog_verbs().iter().map(|(v, d)| (v, d)).collect::<std::collections::HashMap<_,_>>(),
        "recipes": crate::sandbox::recipes().iter().map(|r| serde_json::json!({
            "id": r.id,
            "label": r.label(),
        })).collect::<Vec<_>>(),
        "api": {
            "register": "POST /api/register {name, purpose?} → {token, agent_id, x, y}",
            "view": "GET /api/view?token=[&after_tick=N&timeout_ms=] → {self, view, can, inbox, events} (after_tick = long-poll until tick N+1 lands)",
            "act": "POST /api/act {token, action, base_tick?, seq?} → {ok, accepted, tick, applied_tick, replaced, ticks_behind?, reason?}",
            "catalog": "GET /api/catalog → actions/verbs/recipes (cache once)",
            "message": "POST /api/message {token, targets[], text} → inbox via view",
            "loop": "register once → view → act → view?after_tick=applied_tick → act → …",
        },
    }))
}

// ------------------------------------------------------------------- social

pub(crate) async fn message_send(
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
    if !st
        .rate
        .check(&format!("msg:{}", req.token), 20, Duration::from_secs(60))
    {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("rate_limited".into()),
        });
    }
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

    // StableId → (Entity, Position) for delivery + visibility checks.
    let agents: std::collections::HashMap<u64, (Entity, i32, i32)> = {
        let mut q = sim
            .kernel
            .world
            .query::<(Entity, &StableId, &crate::components::Position, &Agent)>();
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
            let mut counter = sim
                .kernel
                .world
                .resource_mut::<crate::components::MessageCounter>();
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

// --------------------------------------------------------------- spectator

pub(crate) async fn status(State(st): State<AppState>) -> impl IntoResponse {
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

pub(crate) async fn snapshot(
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

pub(crate) async fn agents(State(st): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "agents": st.reg.list_public(),
    }))
}

pub(crate) async fn track(
    State(st): State<AppState>,
    Query(q): Query<TokenQuery>,
) -> impl IntoResponse {
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

pub(crate) async fn art() -> impl IntoResponse {
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

pub(crate) async fn entity(
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

pub(crate) async fn cell(
    State(st): State<AppState>,
    Query(q): Query<CellQuery>,
) -> impl IntoResponse {
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

// ---------------------------------------------------------------------- ops

pub(crate) async fn control(
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
