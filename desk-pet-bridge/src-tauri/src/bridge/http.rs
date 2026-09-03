use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use super::protocol::{parse_incoming, DeskPetEvent};
use crate::cursor::events::map_incoming;
use crate::diagnostics::redact;
use crate::state::{BridgeCommand, Shared};

const BIND: &str = "127.0.0.1:8765";

#[derive(Clone)]
struct HttpState {
    shared: Arc<Shared>,
}

pub async fn serve(shared: Arc<Shared>) {
    let app = Router::new()
        .route("/v1/cursor/event", post(cursor_event))
        .route("/v1/test", post(test_pet))
        .route("/v1/status", get(status))
        .route("/v1/power", post(power))
        .route("/v1/pairing/start", post(pairing_start))
        .route("/v1/pairing/stop", post(pairing_stop))
        .with_state(HttpState {
            shared: shared.clone(),
        });

    let addr: SocketAddr = BIND.parse().expect("static bind address");
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            tracing::info!("loopback API listening on http://{BIND}");
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!("loopback API stopped: {}", redact(&err.to_string()));
            }
        }
        Err(err) => {
            tracing::error!("failed to bind {BIND}: {}", redact(&err.to_string()));
        }
    }
}

async fn cursor_event(State(http): State<HttpState>, Json(body): Json<Value>) -> impl IntoResponse {
    match map_incoming(body).and_then(parse_incoming) {
        Ok(event) => enqueue_or_send(&http.shared, event).await,
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn test_pet(State(http): State<HttpState>, Json(body): Json<Value>) -> impl IntoResponse {
    let state = body
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("coding");
    let mut event = DeskPetEvent::new("sessionStart").with_state(state);
    event.source = Some("test".to_string());
    event.ensure_id();
    match event.validate() {
        Ok(()) => enqueue_or_send(&http.shared, event).await,
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn status(State(http): State<HttpState>) -> impl IntoResponse {
    let snapshot = http.shared.snapshot().await;
    (StatusCode::OK, Json(snapshot))
}

#[derive(Debug, Deserialize)]
struct PowerBody {
    action: String,
}

async fn power(State(http): State<HttpState>, Json(body): Json<PowerBody>) -> impl IntoResponse {
    let event_name = match body.action.as_str() {
        "sleep" => "sleep",
        "wake" => "wake",
        _ => return json_error(StatusCode::BAD_REQUEST, "action must be sleep or wake"),
    };
    let mut event = DeskPetEvent::new(event_name);
    event.ensure_id();
    enqueue_or_send(&http.shared, event).await
}

async fn pairing_start(State(http): State<HttpState>) -> impl IntoResponse {
    let mut event = DeskPetEvent::new("pairingStart");
    event.ensure_id();
    enqueue_or_send(&http.shared, event).await
}

async fn pairing_stop(State(http): State<HttpState>) -> impl IntoResponse {
    let mut event = DeskPetEvent::new("pairingStop");
    event.ensure_id();
    enqueue_or_send(&http.shared, event).await
}

async fn enqueue_or_send(shared: &Shared, event: DeskPetEvent) -> (StatusCode, Json<Value>) {
    shared.record_event(&event.event).await;
    if shared.is_connected().await {
        if shared
            .command_tx
            .send(BridgeCommand::Send(event.clone()))
            .await
            .is_err()
        {
            shared.enqueue(event).await;
            return json_error(StatusCode::SERVICE_UNAVAILABLE, "ble task unavailable");
        }
        (
            StatusCode::ACCEPTED,
            Json(json!({ "ok": true, "queued": false })),
        )
    } else {
        let depth = shared.enqueue(event).await;
        (
            StatusCode::ACCEPTED,
            Json(json!({ "ok": true, "queued": true, "queue": depth })),
        )
    }
}

fn json_error(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "ok": false, "error": redact(message) })),
    )
}
