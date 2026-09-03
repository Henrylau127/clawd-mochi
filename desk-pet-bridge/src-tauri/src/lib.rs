mod ble;
mod bridge;
mod cursor;
mod diagnostics;
mod state;
mod tray;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::mpsc;

use crate::state::{BridgeCommand, Shared};

#[tauri::command]
async fn get_status(shared: tauri::State<'_, Arc<Shared>>) -> Result<state::StatusSnapshot, String> {
    Ok(shared.snapshot().await)
}

#[tauri::command]
async fn reconnect(shared: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    shared
        .command_tx
        .send(BridgeCommand::Reconnect)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn start_pairing(shared: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    shared
        .command_tx
        .send(BridgeCommand::StartPairing)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn stop_pairing(shared: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    shared
        .command_tx
        .send(BridgeCommand::StopPairing)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn test_pet(shared: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    let mut event = bridge::protocol::DeskPetEvent::new("sessionStart").with_state("coding");
    event.source = Some("test".to_string());
    event.ensure_id();
    shared
        .command_tx
        .send(BridgeCommand::Send(event))
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn forget_device(shared: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    shared
        .command_tx
        .send(BridgeCommand::Unbond)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn hide_status(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|err| err.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let (command_tx, command_rx) = mpsc::channel(32);
            let shared = Arc::new(Shared::new(command_tx));
            app.manage(shared.clone());
            tray::setup(app.handle(), shared.clone())?;

            tauri::async_runtime::spawn(bridge::http::serve(shared.clone()));
            tauri::async_runtime::spawn(ble::client::run(shared, command_rx));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            reconnect,
            start_pairing,
            stop_pairing,
            test_pet,
            forget_device,
            hide_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running desk-pet-bridge");
}

#[cfg(test)]
mod hardware_free_tests;
