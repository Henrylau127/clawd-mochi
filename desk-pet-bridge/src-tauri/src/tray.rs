use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

struct PopoverGate {
    closed_at: Option<Instant>,
}

pub fn setup(app: &AppHandle, _shared: Arc<crate::state::Shared>) -> tauri::Result<()> {
    app.manage(Mutex::new(PopoverGate { closed_at: None }));

    let icon = Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))?;
    TrayIconBuilder::with_id("desk-pet")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Desk Pet Bridge")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn note_closed(app: &AppHandle) {
    if let Some(gate) = app.try_state::<Mutex<PopoverGate>>() {
        if let Ok(mut gate) = gate.lock() {
            gate.closed_at = Some(Instant::now());
        }
    }
}

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        note_closed(app);
        let _ = window.hide();
        return;
    }

    if let Some(gate) = app.try_state::<Mutex<PopoverGate>>() {
        if let Ok(gate) = gate.lock() {
            if let Some(closed_at) = gate.closed_at {
                if closed_at.elapsed() < Duration::from_millis(250) {
                    return;
                }
            }
        }
    }

    let _ = window
        .as_ref()
        .window()
        .move_window(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
}
