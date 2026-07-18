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

// -------------------------------------------------- latency-oriented contract

/// The P0 regression: an agent's own act feedback used to die in the
/// 20-tick spectator ring (~1s at test tick rate). The per-agent inbox
/// holds it until the agent views, however late that is.
#[tokio::test]
async fn feedback_survives_agent_think_time() {
    let srv = start().await;
    let reg = register(&srv, "SlowThinker").await;
    let token = reg["token"].as_str().unwrap();

    let (_, a) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": token, "action": {"type": "move", "dx": 1, "dy": 0}})),
    )
    .await;
    assert_eq!(a["accepted"], json!(true));

    // "think" for far longer than EVENT_TICKS_KEPT=20 ticks (20×50ms = 1s)
    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

    let (_, v) = http(&srv, "GET", &format!("/api/view?token={token}"), None).await;
    let events = v["events"].as_array().expect("events array");
    assert!(
        !events.is_empty(),
        "feedback vanished while the agent was thinking"
    );
    assert!(
        events.iter().all(|e| e.get("tick").is_some()),
        "feedback entries must carry their tick stamp: {events:?}"
    );
    // consume-on-read: a second view must not replay them
    let (_, v2) = http(&srv, "GET", &format!("/api/view?token={token}"), None).await;
    let replay = v2["events"].as_array().unwrap();
    assert!(
        replay.len() < events.len() || events.is_empty(),
        "consumed feedback replayed on next view"
    );
}

/// act tells you exactly where your action lands; long-poll view waits for
/// it. One round trip from "I acted" to "here is the result".
#[tokio::test]
async fn act_reports_applied_tick_and_view_long_polls() {
    let srv = start().await;
    let reg = register(&srv, "Precise").await;
    let token = reg["token"].as_str().unwrap();

    let (_, a) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": token, "action": {"type": "idle"}})),
    )
    .await;
    assert_eq!(a["accepted"], json!(true));
    assert_eq!(a["applied_tick"], a["tick"].as_u64().unwrap() + 1);
    assert_eq!(a["replaced"], json!(false));
    let applied = a["applied_tick"].as_u64().unwrap();

    // long-poll returns only once the world has passed `applied`
    let (_, v) = http(
        &srv,
        "GET",
        &format!("/api/view?token={token}&after_tick={applied}"),
        None,
    )
    .await;
    assert!(
        v["tick"].as_u64().unwrap() > applied,
        "long-poll returned before tick {} landed: {}",
        applied,
        v["tick"]
    );

    // after_tick already in the past → immediate answer
    let (_, v2) = http(
        &srv,
        "GET",
        &format!("/api/view?token={token}&after_tick=0"),
        None,
    )
    .await;
    assert_eq!(v2["ok"], json!(true));
}

/// Idempotency: resending the same seq (network retry) cannot double-apply.
#[tokio::test]
async fn act_seq_rejects_duplicates() {
    let srv = start().await;
    let reg = register(&srv, "Retry").await;
    let token = reg["token"].as_str().unwrap();
    let act = |seq: u64| {
        json!({"token": token, "seq": seq, "action": {"type": "idle"}})
    };

    let (_, a1) = http(&srv, "POST", "/api/act", Some(act(7))).await;
    assert_eq!(a1["accepted"], json!(true));
    let (_, a2) = http(&srv, "POST", "/api/act", Some(act(7))).await;
    assert_eq!(a2["accepted"], json!(false));
    assert_eq!(a2["reason"], json!("duplicate_seq"));
    let (_, a3) = http(&srv, "POST", "/api/act", Some(act(3))).await;
    assert_eq!(a3["reason"], json!("duplicate_seq"), "older seq must also reject");
    let (_, a4) = http(&srv, "POST", "/api/act", Some(act(8))).await;
    assert_eq!(a4["accepted"], json!(true));
}

/// base_tick is a soft staleness signal: the act still lands, but the
/// response says how far behind the deciding view was.
#[tokio::test]
async fn act_base_tick_reports_ticks_behind() {
    let srv = start().await;
    let reg = register(&srv, "Stale").await;
    let token = reg["token"].as_str().unwrap();

    let (_, a) = http(
        &srv,
        "POST",
        "/api/act",
        Some(json!({"token": token, "base_tick": 0, "action": {"type": "idle"}})),
    )
    .await;
    assert_eq!(a["accepted"], json!(true), "stale view must not reject the act");
    assert_eq!(
        a["ticks_behind"].as_u64().unwrap(),
        a["tick"].as_u64().unwrap(),
        "ticks_behind = tick - base_tick"
    );
}

// -------------------------------------------------- message delivery receipts

/// Per-target receipts: each accepted target gets a msg_id, each rejected
/// one gets the reason — no more silent aggregate counts.
#[tokio::test]
async fn message_send_reports_per_target_results() {
    let srv = start().await;
    let a = register(&srv, "Recv A").await;
    let b = register(&srv, "Recv B").await;
    let (id_a, id_b) = (
        a["agent_id"].as_u64().unwrap(),
        b["agent_id"].as_u64().unwrap(),
    );

    let (_, v) = http(
        &srv,
        "POST",
        "/api/message",
        Some(json!({
            "token": srv.dev_token,
            "targets": [id_a, id_b, 999_999],
            "text": "hello from ops",
        })),
    )
    .await;
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["sent"], json!(2));
    assert_eq!(v["rejected"], json!(1));
    let results = v["results"].as_array().expect("results array");
    assert_eq!(results.len(), 3);
    let ok_rows: Vec<_> = results.iter().filter(|r| r["ok"] == json!(true)).collect();
    assert_eq!(ok_rows.len(), 2);
    assert!(
        ok_rows.iter().all(|r| r["msg_id"].is_u64()),
        "accepted targets must carry msg_id for status polling"
    );
    let bad = results.iter().find(|r| r["ok"] == json!(false)).unwrap();
    assert_eq!(bad["id"], json!(999_999));
    assert_eq!(bad["reason"], json!("unknown_target"));
}

/// The receipt loop: unread after send, read_tick stamped once the
/// recipient's next view consumes its inbox. Gated: other tokens see
/// nothing, the sender (here dev) sees its own.
#[tokio::test]
async fn message_status_tracks_read_receipt() {
    let srv = start().await;
    let reg = register(&srv, "Reader").await;
    let token = reg["token"].as_str().unwrap();
    let id = reg["agent_id"].as_u64().unwrap();

    let (_, v) = http(
        &srv,
        "POST",
        "/api/message",
        Some(json!({"token": srv.dev_token, "targets": [id], "text": "ping"})),
    )
    .await;
    let msg_id = v["results"][0]["msg_id"].as_u64().unwrap();
    let status_url = |ids: &str| {
        format!("/api/message/status?token={}&ids={ids}", srv.dev_token)
    };

    // before the agent views: delivered but unread
    let (_, s0) = http(&srv, "GET", &status_url(&msg_id.to_string()), None).await;
    let row = &s0["statuses"][0];
    assert_eq!(row["id"], json!(msg_id));
    assert!(row["read_tick"].is_null(), "read before any view?");

    // agent views → inbox consumed → ledger stamped
    let (_, _view) = http(&srv, "GET", &format!("/api/view?token={token}"), None).await;
    let (_, s1) = http(&srv, "GET", &status_url(&msg_id.to_string()), None).await;
    assert!(
        s1["statuses"][0]["read_tick"].is_u64(),
        "read_tick not stamped after view: {s1}"
    );

    // gating: the recipient's token is NOT the sender — sees nothing
    let (_, s2) = http(
        &srv,
        "GET",
        &format!("/api/message/status?token={token}&ids={msg_id}"),
        None,
    )
    .await;
    assert_eq!(
        s2["statuses"].as_array().unwrap().len(),
        0,
        "non-sender token leaked a receipt"
    );
}

/// Target 0 = operator: agents can always reach it (no FOV gate), the
/// dev token drains it, consume-on-read, non-dev is refused.
#[tokio::test]
async fn operator_inbox_receives_agent_replies() {
    let srv = start().await;
    let reg = register(&srv, "Reporter").await;
    let token = reg["token"].as_str().unwrap();

    let (_, v) = http(
        &srv,
        "POST",
        "/api/message",
        Some(json!({"token": token, "targets": [0], "text": "ore depleted, new orders?"})),
    )
    .await;
    assert_eq!(v["sent"], json!(1), "target 0 must always be deliverable");

    // non-dev cannot read the operator inbox
    let (_, nope) = http(
        &srv,
        "GET",
        &format!("/api/message/inbox?token={token}"),
        None,
    )
    .await;
    assert_eq!(nope["reason"], json!("dev_token_required"));

    // dev drains it
    let (_, inbox) = http(
        &srv,
        "GET",
        &format!("/api/message/inbox?token={}", srv.dev_token),
        None,
    )
    .await;
    let msgs = inbox["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["from"], json!("Reporter"));
    assert_eq!(msgs[0]["text"], json!("ore depleted, new orders?"));

    // consume-on-read: second drain is empty
    let (_, again) = http(
        &srv,
        "GET",
        &format!("/api/message/inbox?token={}", srv.dev_token),
        None,
    )
    .await;
    assert_eq!(again["messages"].as_array().unwrap().len(), 0);
}
