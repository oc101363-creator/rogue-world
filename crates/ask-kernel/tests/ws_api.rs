//! WebSocket end-to-end tests — real server + tokio-tungstenite client.
//! Verifies per-subscription FOV gating and WS-side auth rules.

use std::net::SocketAddr;
use std::time::Duration;

use ask_kernel::config::Config;
use ask_kernel::serve::Serve;
use ask_kernel::world::KernelWorld;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

async fn start() -> (SocketAddr, Serve, tokio::task::JoinHandle<()>) {
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 11;
    let kernel = KernelWorld::new(&cfg);
    let serve = Serve::build(kernel, 50, None);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = serve.router();
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (addr, serve, handle)
}

fn decode_feats(snap: &Value) -> Vec<u16> {
    use base64::Engine;
    let data = snap["feat_ids"]["data"].as_str().unwrap_or("");
    let raw = base64::engine::general_purpose::STANDARD
        .decode(data)
        .unwrap_or_default();
    raw.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

async fn recv_snapshot(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("ws timeout")
        .expect("ws closed")
        .expect("ws error");
    let Message::Text(t) = msg else {
        panic!("non-text ws frame");
    };
    serde_json::from_str(&t).expect("snapshot json")
}

#[tokio::test]
async fn ws_dark_until_subscribed_then_dev_sees_all() {
    let (addr, serve, _h) = start().await;
    let url = format!("ws://{addr}/ws");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // no subscription → fully masked
    let snap = recv_snapshot(&mut ws).await;
    let feats = decode_feats(&snap);
    assert!(
        feats.iter().all(|&f| f == 0),
        "unsubscribed ws leaked terrain"
    );

    // subscribe with the dev token → full map
    ws.send(Message::Text(
        json!({"type": "subscribe", "tokens": [serve.dev_token()]})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let snap = recv_snapshot(&mut ws).await;
    let feats = decode_feats(&snap);
    let nonzero = feats.iter().filter(|&&f| f != 0).count();
    assert!(nonzero > 1000, "dev subscription should reveal the map");
}

#[tokio::test]
async fn ws_action_requires_token() {
    let (addr, _serve, _h) = start().await;

    // register an agent over HTTP first
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let body = json!({"name": "WsMover", "purpose": "ws test"}).to_string();
    let req = format!(
        "POST /api/register HTTP/1.0\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let body_start = text.find("\r\n\r\n").unwrap() + 4;
    let reg: Value = serde_json::from_str(&text[body_start..]).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();

    let url = format!("ws://{addr}/ws");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.send(Message::Text(
        json!({"type": "subscribe", "tokens": [token], "focus": token})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let _ = recv_snapshot(&mut ws).await;

    // action WITHOUT token → must be ignored
    for _ in 0..4 {
        ws.send(Message::Text(
            json!({"type": "action", "action": {"type": "move", "dx": 1, "dy": 0}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    }
    // action WITHOUT token → must be ignored. Assert NO self-move event for
    // our agent across several frames (position-based checks are flaky: the
    // world fights back — monsters/traps can relocate an idle agent).
    for _ in 0..3 {
        let snap = recv_snapshot(&mut ws).await;
        let self_moved = snap["recent_events"]
            .as_array()
            .map(|evs| {
                evs.iter().any(|e| {
                    e["type"].as_str() == Some("moved")
                        && e["entity"].as_u64() == reg["agent_id"].as_u64()
                })
            })
            .unwrap_or(false);
        assert!(!self_moved, "tokenless ws action produced a move event");
    }

    // action WITH token → applied. Try the four directions until one is not
    // blocked by terrain (spawn cells can be walled on some sides). Watch
    // for the `moved` EVENT — position diffs can lie (death respawn).
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut moved = false;
    for i in 0..16 {
        let (dx, dy) = dirs[i % 4];
        ws.send(Message::Text(
            json!({"type": "action", "token": token, "action": {"type": "move", "dx": dx, "dy": dy}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        let snap = recv_snapshot(&mut ws).await;
        let self_moved = snap["recent_events"]
            .as_array()
            .map(|evs| {
                evs.iter().any(|e| {
                    e["type"].as_str() == Some("moved")
                        && e["entity"].as_u64() == reg["agent_id"].as_u64()
                })
            })
            .unwrap_or(false);
        if self_moved {
            moved = true;
            break;
        }
    }
    assert!(moved, "token ws action never moved the agent in 16 tries");
}
