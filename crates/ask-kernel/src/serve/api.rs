//! HTTP handlers — one thin function per endpoint. Handlers never reach
//! into gameplay internals; they resolve identity, call a projection
//! (agent_view / viewer / inspect), and serialize.

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use bevy_ecs::prelude::{Entity, With};
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    tick: Option<u64>,
    action: Action,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActionResponse {
    ok: bool,
    accepted: bool,
    tick: u64,
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
    Query(q): Query<TokenQuery>,
) -> impl IntoResponse {
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
    match crate::agent_view::build_agent_view(&mut sim.kernel.world, agent_e, &recent) {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "ok": false, "reason": "no_agent" })),
    }
}

pub(crate) async fn act(
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
            "view": "GET /api/view?token= → {self, view, can, inbox, events}",
            "act": "POST /api/act {token, action} → {ok, accepted, tick, reason?}",
            "catalog": "GET /api/catalog → actions/verbs/recipes (cache once)",
            "message": "POST /api/message {token, targets[], text} → inbox via view",
            "loop": "register once → view → act → view …",
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
