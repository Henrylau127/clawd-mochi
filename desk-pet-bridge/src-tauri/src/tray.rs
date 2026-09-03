use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::bridge::protocol::DeskPetEvent;
use crate::state::{BridgeCommand, Shared};

pub fn setup(app: &AppHandle, shared: Arc<Shared>) -> tauri::Result<()> {
    let reconnect = MenuItemBuilder::with_id("reconnect", "Reconnect").build(app)?;
    let start_pairing = MenuItemBuilder::with_id("start-pairing", "Start pairing").build(app)?;
    let stop_pairing = MenuItemBuilder::with_id("stop-pairing", "Stop pairing").build(app)?;
    let test_pet = MenuItemBuilder::with_id("test-pet", "Test pet").build(app)?;
    let show_status = MenuItemBuilder::with_id("show-status", "Show status").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&reconnect)
        .item(&start_pairing)
        .item(&stop_pairing)
        .separator()
        .item(&test_pet)
        .item(&show_status)
        .separator()
        .item(&quit)
        .build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))?;
    let shared_for_menu = shared.clone();
    TrayIconBuilder::with_id("desk-pet")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Desk Pet Bridge")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let shared = shared_for_menu.clone();
            let app = app.clone();
            let id = event.id().as_ref().to_string();
            tauri::async_runtime::spawn(async move {
                handle_menu(&app, &shared, &id).await;
            });
        })
        .build(app)?;

    Ok(())
}

async fn handle_menu(app: &AppHandle, shared: &Shared, id: &str) {
    match id {
        "reconnect" => {
            let _ = shared.command_tx.send(BridgeCommand::Reconnect).await;
        }
        "start-pairing" => {
            let _ = shared.command_tx.send(BridgeCommand::StartPairing).await;
        }
        "stop-pairing" => {
            let _ = shared.command_tx.send(BridgeCommand::StopPairing).await;
        }
        "test-pet" => {
            let mut event = DeskPetEvent::new("sessionStart").with_state("coding");
            event.source = Some("test".to_string());
            event.ensure_id();
            let _ = shared.command_tx.send(BridgeCommand::Send(event)).await;
        }
        "show-status" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}
