mod ble;
mod bridge;
mod cursor;
mod diagnostics;
mod glass;
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
async fn set_loopback_port(
    shared: tauri::State<'_, Arc<Shared>>,
    port: u16,
) -> Result<u16, String> {
    let listening = shared.is_listening_on(port).await;
    let port = crate::bridge::loopback::reject_if_occupied(port, listening)?;
    if listening {
        crate::bridge::loopback::save_port(port)?;
        return Ok(port);
    }
    shared.request_port(port)?;
    crate::bridge::loopback::save_port(port)?;
    Ok(port)
}

#[tauri::command]
fn hide_status(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|err| err.to_string())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
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
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let (command_tx, command_rx) = mpsc::channel(32);
            let shared = Arc::new(Shared::new(command_tx));
            app.manage(shared.clone());
            tray::setup(app.handle(), shared.clone())?;
            if let Some(window) = app.get_webview_window("main") {
                crate::glass::apply_menu_glass(&window);
            }

            tauri::async_runtime::spawn(bridge::http::serve(shared.clone()));
            tauri::async_runtime::spawn(ble::client::run(shared, command_rx));
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let _ = window.hide();
                    api.prevent_close();
                }
                tauri::WindowEvent::Focused(false) => {
                    crate::tray::note_closed(window.app_handle());
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            reconnect,
            start_pairing,
            stop_pairing,
            test_pet,
            forget_device,
            set_loopback_port,
            hide_status,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running desk-pet-bridge");
}

#[cfg(test)]
mod hardware_free_tests;
