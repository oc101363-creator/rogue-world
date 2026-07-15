//! HTTP + WebSocket observer gateway (kernel stays free of UI frameworks).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::config::Config;
use crate::events::EventBuf;
use crate::tick::Sim;
use crate::viewer::{build_viewer_snapshot, ViewerSnapshot};
use crate::world::KernelWorld;

#[derive(Clone)]
struct AppState {
    latest: Arc<RwLock<ViewerSnapshot>>,
    tx: broadcast::Sender<String>,
}

pub async fn run_server(port: u16, tick_ms: u64, cfg: Config) -> Result<()> {
    let kernel = KernelWorld::new(&cfg);
    let mut sim = Sim::new(kernel);

    let initial = build_viewer_snapshot(&mut sim.kernel.world, &[]);
    let (tx, _rx) = broadcast::channel::<String>(64);
    let latest = Arc::new(RwLock::new(initial));

    // Dedicated OS thread owns the sim (bevy World stays single-threaded).
    let latest_thread = latest.clone();
    let tx_thread = tx.clone();
    std::thread::spawn(move || {
        let mut recent: Vec<crate::events::GameEvent> = Vec::new();
        loop {
            sim.step();
            let mut ev = sim.kernel.world.resource_mut::<EventBuf>().drain();
            recent.extend(ev.drain(..));
            if recent.len() > 40 {
                let drain_n = recent.len() - 40;
                recent.drain(0..drain_n);
            }
            let snap = build_viewer_snapshot(&mut sim.kernel.world, &recent);
            let json = serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into());
            {
                let mut g = latest_thread.blocking_write();
                *g = snap;
            }
            let _ = tx_thread.send(json);
            std::thread::sleep(Duration::from_millis(tick_ms));
        }
    });

    let state = AppState { latest, tx };

    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let index = static_dir.join("index.html");

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/snapshot", get(api_snapshot))
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

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("[ask] viewer  http://{addr}/");
    eprintln!("[ask] ws      ws://{addr}/ws");
    eprintln!("[ask] status  http://{addr}/api/status");
    eprintln!("[ask] mock agent ticking every {tick_ms}ms");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_status(State(st): State<AppState>) -> impl IntoResponse {
    let snap = st.latest.read().await;
    Json(serde_json::json!({
        "ok": true,
        "service": "ask-kernel",
        "tick": snap.tick,
        "width": snap.width,
        "height": snap.height,
        "agents": snap.entities.iter().filter(|e| e.kind == "agent").count(),
    }))
}

async fn api_snapshot(State(st): State<AppState>) -> impl IntoResponse {
    let snap = st.latest.read().await;
    Json(snap.clone())
}

async fn ws_handler(ws: WebSocketUpgrade, State(st): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_ws(socket, st))
}

async fn client_ws(mut socket: WebSocket, st: AppState) {
    {
        let snap = st.latest.read().await;
        if let Ok(s) = serde_json::to_string(&*snap) {
            let _ = socket.send(Message::Text(s.into())).await;
        }
    }

    let mut rx = st.tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => {
                        let _ = socket.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Text(_))) => {}
                    _ => {}
                }
            }
        }
    }
}
