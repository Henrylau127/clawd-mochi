use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;

use super::loopback::{self, DEFAULT_PORT, HOST};
use super::protocol::{parse_incoming, DeskPetEvent};
use crate::codex::events::map_incoming as map_codex;
use crate::cursor::events::map_incoming;
use crate::diagnostics::redact;
use crate::omp::events::map_incoming as map_omp;
use crate::state::{BridgeCommand, Shared};

#[derive(Clone)]
struct HttpState {
    shared: Arc<Shared>,
}

pub fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/v1/cursor/event", post(cursor_event))
        .route("/v1/codex/event", post(codex_event))
        .route("/v1/omp/event", post(omp_event))
        .route("/v1/test", post(test_pet))
        .route("/v1/status", get(status))
        .route("/v1/power", post(power))
        .route("/v1/pairing/start", post(pairing_start))
        .route("/v1/pairing/stop", post(pairing_stop))
        .with_state(HttpState { shared })
}

pub async fn bind_exact(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await
}

pub async fn bind_preferred(port: u16) -> std::io::Result<(TcpListener, u16, bool)> {
    let mut last_err: Option<std::io::Error> = None;
    for offset in 0..16u16 {
        let Some(candidate) = port.checked_add(offset) else {
            break;
        };
        match bind_exact(candidate).await {
            Ok(listener) => return Ok((listener, candidate, offset > 0)),
            Err(err) if err.kind() == ErrorKind::AddrInUse => last_err = Some(err),
            Err(err) => return Err(err),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(ErrorKind::AddrInUse, "no free loopback port nearby")
    }))
}

pub async fn serve(shared: Arc<Shared>) {
    let mut port_rx = shared.subscribe_port();
    let mut allow_nearby = true;

    loop {
        // Mark the current port seen. `borrow()` leaves the receiver dirty, so
        // graceful shutdown would fire immediately and spin-bind forever.
        let requested = *port_rx.borrow_and_update();
        let bound = if allow_nearby {
            allow_nearby = false;
            bind_preferred(requested)
                .await
                .map(|(listener, actual, moved)| (listener, actual, moved))
        } else {
            bind_exact(requested)
                .await
                .map(|listener| (listener, requested, false))
        };

        match bound {
            Ok((listener, actual, moved)) => {
                let addr = listener
                    .local_addr()
                    .map(|socket| socket.to_string())
                    .unwrap_or_else(|_| format!("{HOST}:{actual}"));
                let note = if moved {
                    Some(format!(
                        "Port {requested} was in use. Listening on {actual}."
                    ))
                } else {
                    None
                };
                shared
                    .set_loopback(Some(addr.clone()), actual, None, note)
                    .await;
                if !cfg!(test) {
                    let _ = loopback::save_port(actual);
                }
                tracing::info!("loopback API listening on http://{addr}");

                let mut wait_change = port_rx.clone();
                let shutdown = async move {
                    let _ = wait_change.changed().await;
                };
                if let Err(err) = axum::serve(listener, router(shared.clone()))
                    .with_graceful_shutdown(shutdown)
                    .await
                {
                    tracing::error!("loopback API stopped: {}", redact(&err.to_string()));
                    return;
                }
            }
            Err(err) => {
                let message = format!("Port {requested} is unavailable.");
                tracing::error!(
                    "failed to bind {HOST}:{requested}: {}",
                    redact(&err.to_string())
                );
                shared
                    .set_loopback(None, requested, Some(message), None)
                    .await;
                if port_rx.changed().await.is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn serve_with_listener(shared: Arc<Shared>, listener: TcpListener) {
    let addr = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| format!("{HOST}:{DEFAULT_PORT}"));
    let port = listener
        .local_addr()
        .map(|addr: SocketAddr| addr.port())
        .unwrap_or(DEFAULT_PORT);
    shared
        .set_loopback(Some(addr.clone()), port, None, None)
        .await;
    tracing::info!("loopback API listening on http://{addr}");
    if let Err(err) = axum::serve(listener, router(shared)).await {
        tracing::error!("loopback API stopped: {}", redact(&err.to_string()));
    }
}

async fn cursor_event(State(http): State<HttpState>, Json(body): Json<Value>) -> impl IntoResponse {
    match map_incoming(body).and_then(parse_incoming) {
        Ok(event) => enqueue_or_send(&http.shared, event).await,
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn codex_event(State(http): State<HttpState>, Json(body): Json<Value>) -> impl IntoResponse {
    match map_codex(body).and_then(parse_incoming) {
        Ok(event) => enqueue_or_send(&http.shared, event).await,
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn omp_event(State(http): State<HttpState>, Json(body): Json<Value>) -> impl IntoResponse {
    match map_omp(body).and_then(parse_incoming) {
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
    let snapshot = http.shared.public_snapshot().await;
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
    request_command(&http.shared, BridgeCommand::StartPairing).await
}

async fn pairing_stop(State(http): State<HttpState>) -> impl IntoResponse {
    request_command(&http.shared, BridgeCommand::StopPairing).await
}

async fn request_command(shared: &Shared, command: BridgeCommand) -> (StatusCode, Json<Value>) {
    match shared.command_tx.send(command).await {
        Ok(()) => (StatusCode::ACCEPTED, Json(json!({ "ok": true }))),
        Err(err) => json_error(StatusCode::SERVICE_UNAVAILABLE, &err.to_string()),
    }
}

async fn enqueue_or_send(shared: &Shared, event: DeskPetEvent) -> (StatusCode, Json<Value>) {
    let control = match event.event.as_str() {
        "pairingStart" => Some(BridgeCommand::StartPairing),
        "pairingStop" => Some(BridgeCommand::StopPairing),
        "unbond" => Some(BridgeCommand::Unbond),
        _ => None,
    };
    if let Some(command) = control {
        return request_command(shared, command).await;
    }

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
