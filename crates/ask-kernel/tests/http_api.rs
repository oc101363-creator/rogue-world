//! HTTP API end-to-end tests — real server on an ephemeral port, raw HTTP/1.0
//! over TcpStream (no client deps). Covers the auth rules that used to be holes.

use std::net::SocketAddr;

use ask_kernel::config::Config;
use ask_kernel::serve::Serve;
use ask_kernel::world::KernelWorld;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct TestServer {
    addr: SocketAddr,
    dev_token: String,
    _serve: Serve,
    _handle: tokio::task::JoinHandle<()>,
}

async fn start() -> TestServer {
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 7;
    let kernel = KernelWorld::new(&cfg);
    let serve = Serve::build(kernel, 50, None);
    let dev_token = serve.dev_token();
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
    TestServer {
        addr,
        dev_token,
        _serve: serve,
        _handle: handle,
    }
}

/// Minimal HTTP/1.0 exchange (close-delimited body, no chunked encoding).
async fn http(srv: &TestServer, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let mut s = TcpStream::connect(srv.addr).await.unwrap();
    let body_s = body.map(|b| b.to_string()).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.0\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body_s}",
        body_s.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(text.len());
    let json = serde_json::from_str(&text[body_start..]).unwrap_or(json!(null));
    (status, json)
}

async fn register(srv: &TestServer, name: &str) -> Value {
    let (_, v) = http(
        srv,
        "POST",
        "/api/register",
        Some(json!({"name": name, "purpose": "test"})),
    )
    .await;
    assert_eq!(v["ok"], json!(true), "register failed: {v}");
    v
}

#[tokio::test]
async fn register_view_act_roundtrip() {
    let srv = start().await;
    let reg = register(&srv, "HttpTest").await;
    let token = reg["token"].as_str().unwrap();

    // canonical view shape (no legacy flat aliases)
    let (_, v) = http(&srv, "GET", &format!("/api/view?token={token}"), None).await;
    assert_eq!(v["ok"], json!(true));
    for k in ["self", "view", "can", "inbox", "events"] {
        assert!(v.get(k).is_some(), "missing canonical key {k}");
    }
    assert!(v.get("interactions").is_none(), "flat alias leaked");
    assert_eq!(v["self"]["name"], json!("HttpTest"));

    // act with token accepted
    let (_, v) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": token, "action": {"type": "move", "dx": 1, "dy": 0}})),
    )
    .await;
    assert_eq!(v["accepted"], json!(true));
}

#[tokio::test]
async fn act_requires_token_and_rejects_mismatch() {
    let srv = start().await;
    let reg = register(&srv, "Guarded").await;
    let token = reg["token"].as_str().unwrap();
    let other = register(&srv, "Other").await;
    let other_id = other["agent_id"].as_u64().unwrap();

    // bare agent_id: rejected (the old free-for-all hole)
    let (_, v) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"agent_id": other_id, "action": {"type": "idle"}})),
    )
    .await;
    assert_eq!(v["accepted"], json!(false));
    assert_eq!(v["reason"], json!("token_required"));

    // mismatched token+id pair: rejected
    let (_, v) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": token, "agent_id": other_id, "action": {"type": "idle"}})),
    )
    .await;
    assert_eq!(v["reason"], json!("agent_id_token_mismatch"));

    // bogus token: rejected
    let (_, v) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": "ask1_nope", "action": {"type": "idle"}})),
    )
    .await;
    assert_eq!(v["reason"], json!("invalid_token"));
}

#[tokio::test]
async fn control_requires_dev_token() {
    let srv = start().await;

    // no token: rejected
    let (_, v) = http(
        &srv,
        "POST",
        "/api/control",
        Some(json!({"human_control": true})),
    )
    .await;
    assert_eq!(v["ok"], json!(false));
    assert_eq!(v["reason"], json!("dev_token_required"));

    // dev token: accepted
    let (_, v) = http(
        &srv,
        "POST",
        "/api/control",
        Some(json!({"human_control": true, "token": srv.dev_token})),
    )
    .await;
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["human_control"], json!(true));
}

#[tokio::test]
async fn register_is_rate_limited() {
    let srv = start().await;
    let mut reasons = Vec::new();
    for i in 0..12 {
        let (_, v) = http(
            &srv,
            "POST",
            "/api/register",
            Some(json!({"name": format!("Bot{i}")})),
        )
        .await;
        reasons.push(v["reason"].as_str().unwrap_or("ok").to_string());
    }
    let limited = reasons.iter().filter(|r| r.as_str() == "rate_limited").count();
    assert!(limited >= 1, "register spam never throttled: {reasons:?}");
}
