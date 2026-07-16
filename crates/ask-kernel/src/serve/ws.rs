//! WebSocket loop — one task per client; subscribe → per-tick snapshots.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;

use super::{build_snapshot_for_tokens, recent_snapshot, AppState};
use crate::actions::Action;
use crate::grid::Grid;
use crate::viewer::build_viewer_snapshot_with;
use crate::vision::VisionMap;

pub(crate) async fn handler(
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
                                "action" => handle_action(&st, &v).await,
                                "control" => handle_control(&st, &v),
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

async fn handle_action(st: &AppState, v: &serde_json::Value) {
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
    if super::api::validate_action(&action).is_err() {
        return;
    }
    st.bus.submit(Some(agent_id), action, Some(tick));
}

fn handle_control(st: &AppState, v: &serde_json::Value) {
    // World-wide switches are operator-only: dev token required.
    let token = v.get("token").and_then(|x| x.as_str()).unwrap_or("");
    if !st.reg.is_dev_token(token) {
        return;
    }
    if let Some(hc) = v.get("human_control").and_then(|x| x.as_bool()) {
        st.bus.set_human_control(hc);
    }
}
