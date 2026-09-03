use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::state::{
    BlePhase, EnrollmentPhase, SecureSessionState, Shared, StatusSnapshot,
};

struct PopoverGate {
    closed_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayStatusKind {
    Ready,
    Connecting,
    Offline,
    Idle,
}

pub fn setup(app: &AppHandle, shared: Arc<Shared>) -> tauri::Result<()> {
    app.manage(Mutex::new(PopoverGate { closed_at: None }));

    let icon = Image::from_bytes(include_bytes!("../icons/trayIdle.png"))?;
    TrayIconBuilder::with_id("desk-pet")
        .icon(icon)
        // Colored status dots require a non-template image on macOS.
        .icon_as_template(false)
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

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        mirror_status_to_tray(app_handle, shared).await;
    });

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

async fn mirror_status_to_tray(app: AppHandle, shared: Arc<Shared>) {
    let mut last: Option<(TrayStatusKind, String)> = None;
    loop {
        let snapshot = shared.snapshot().await;
        let kind = tray_status_kind(&snapshot);
        let tip = tray_tooltip(&snapshot);
        let next = (kind, tip);
        if last.as_ref() != Some(&next) {
            apply_tray_appearance(&app, next.0, &next.1);
            last = Some(next);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn apply_tray_appearance(app: &AppHandle, kind: TrayStatusKind, tooltip: &str) {
    let Some(tray) = app.tray_by_id("desk-pet") else {
        return;
    };
    let bytes = icon_bytes(kind);
    let Ok(icon) = Image::from_bytes(bytes) else {
        return;
    };
    let _ = tray.set_icon_with_as_template(Some(icon), false);
    let _ = tray.set_tooltip(Some(tooltip));
}

fn icon_bytes(kind: TrayStatusKind) -> &'static [u8] {
    match kind {
        TrayStatusKind::Ready => include_bytes!("../icons/trayReady.png"),
        TrayStatusKind::Connecting => include_bytes!("../icons/trayConnecting.png"),
        TrayStatusKind::Offline => include_bytes!("../icons/trayOffline.png"),
        TrayStatusKind::Idle => include_bytes!("../icons/trayIdle.png"),
    }
}

fn tray_status_kind(snapshot: &StatusSnapshot) -> TrayStatusKind {
    if snapshot.ble == BlePhase::Connected
        && snapshot.secure_session == SecureSessionState::Ready
        && snapshot.link_verified
    {
        return TrayStatusKind::Ready;
    }

    if snapshot.ble == BlePhase::Connected
        && snapshot.secure_session == SecureSessionState::Ready
        && !snapshot.link_verified
    {
        return TrayStatusKind::Connecting;
    }

    let connecting = snapshot.secure_session == SecureSessionState::Handshaking
        || snapshot.ble == BlePhase::Scanning
        || snapshot.pairing_active
        || matches!(
            snapshot.enrollment,
            EnrollmentPhase::AwaitingDevicePair
                | EnrollmentPhase::Scanning
                | EnrollmentPhase::PasskeyPending
        );
    if connecting {
        return TrayStatusKind::Connecting;
    }

    if snapshot.active_pet_id.is_some() || !snapshot.pets.is_empty() {
        TrayStatusKind::Offline
    } else {
        TrayStatusKind::Idle
    }
}

fn tray_tooltip(snapshot: &StatusSnapshot) -> String {
    let pet = snapshot
        .device
        .as_deref()
        .or_else(|| {
            let id = snapshot.active_pet_id.as_deref()?;
            snapshot
                .pets
                .iter()
                .find(|pet| pet.id == id || pet.name == id)
                .map(|pet| pet.name.as_str())
        })
        .or_else(|| snapshot.pets.first().map(|pet| pet.name.as_str()));

    match tray_status_kind(snapshot) {
        TrayStatusKind::Ready => match pet {
            Some(name) => format!("Connected · {name}"),
            None => "Connected".to_string(),
        },
        TrayStatusKind::Connecting if snapshot.ble == BlePhase::Connected
            && snapshot.secure_session == SecureSessionState::Ready =>
        {
            match pet {
                Some(name) => format!("Verifying link · {name}…"),
                None => "Verifying pet link…".to_string(),
            }
        }
        TrayStatusKind::Connecting => {
            if snapshot.secure_session == SecureSessionState::Handshaking {
                match pet {
                    Some(name) => format!("Securing {name}…"),
                    None => "Securing pet…".to_string(),
                }
            } else if snapshot.pairing_active
                || matches!(
                    snapshot.enrollment,
                    EnrollmentPhase::AwaitingDevicePair | EnrollmentPhase::PasskeyPending
                )
            {
                "Pairing…".to_string()
            } else {
                match pet {
                    Some(name) => format!("Looking for {name}…"),
                    None => "Looking for a pet…".to_string(),
                }
            }
        }
        TrayStatusKind::Offline => {
            let base = match pet {
                Some(name) => format!("{name} is offline"),
                None => "Pet offline".to_string(),
            };
            match snapshot.last_error.as_deref() {
                Some(err) if !err.is_empty() => format!("{base} · {err}"),
                _ => base,
            }
        }
        TrayStatusKind::Idle => "Desk Pet Bridge · no pet paired".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::loopback::KnownPet;

    fn snap(
        ble: BlePhase,
        secure: SecureSessionState,
        enrollment: EnrollmentPhase,
        device: Option<&str>,
        pets: Vec<KnownPet>,
        active: Option<&str>,
        pairing_active: bool,
        last_error: Option<&str>,
        link_verified: bool,
    ) -> StatusSnapshot {
        StatusSnapshot {
            ble,
            secure_session: secure,
            enrollment,
            device: device.map(str::to_string),
            queue: 0,
            queue_items: Vec::new(),
            pairing_active,
            pairing_remaining_seconds: pairing_active.then_some(30),
            last_event: None,
            last_error: last_error.map(str::to_string),
            pet_mode: None,
            pet_state: None,
            loopback_host: "127.0.0.1".into(),
            loopback_port: 6900,
            loopback_addr: "127.0.0.1:6900".into(),
            loopback_error: None,
            loopback_note: None,
            provider: "cursor".into(),
            pets,
            active_pet_id: active.map(str::to_string),
            link_verified,
            ts: 0,
        }
    }

    #[test]
    fn maps_ready_connecting_offline_and_idle() {
        let pet = KnownPet {
            id: "abc".into(),
            name: "DP-CC98".into(),
        };
        assert_eq!(
            tray_status_kind(&snap(
                BlePhase::Connected,
                SecureSessionState::Ready,
                EnrollmentPhase::Idle,
                Some("DP-CC98"),
                vec![pet.clone()],
                Some("abc"),
                false,
                None,
                true,
            )),
            TrayStatusKind::Ready
        );
        assert_eq!(
            tray_status_kind(&snap(
                BlePhase::Connected,
                SecureSessionState::Ready,
                EnrollmentPhase::Idle,
                Some("DP-CC98"),
                vec![pet.clone()],
                Some("abc"),
                false,
                None,
                false,
            )),
            TrayStatusKind::Connecting
        );
        assert_eq!(
            tray_status_kind(&snap(
                BlePhase::Scanning,
                SecureSessionState::Handshaking,
                EnrollmentPhase::Idle,
                Some("DP-CC98"),
                vec![pet.clone()],
                Some("abc"),
                false,
                None,
                false,
            )),
            TrayStatusKind::Connecting
        );
        assert_eq!(
            tray_status_kind(&snap(
                BlePhase::Disconnected,
                SecureSessionState::Disconnected,
                EnrollmentPhase::Idle,
                None,
                vec![pet],
                Some("abc"),
                false,
                None,
                false,
            )),
            TrayStatusKind::Offline
        );
        assert_eq!(
            tray_status_kind(&snap(
                BlePhase::Disconnected,
                SecureSessionState::Disconnected,
                EnrollmentPhase::Idle,
                None,
                Vec::new(),
                None,
                false,
                None,
                false,
            )),
            TrayStatusKind::Idle
        );
    }

    #[test]
    fn tooltip_names_the_active_pet() {
        let pet = KnownPet {
            id: "abc".into(),
            name: "DP-CC98".into(),
        };
        assert_eq!(
            tray_tooltip(&snap(
                BlePhase::Connected,
                SecureSessionState::Ready,
                EnrollmentPhase::Idle,
                Some("DP-CC98"),
                vec![pet.clone()],
                Some("abc"),
                false,
                None,
                true,
            )),
            "Connected · DP-CC98"
        );
        assert_eq!(
            tray_tooltip(&snap(
                BlePhase::Scanning,
                SecureSessionState::Handshaking,
                EnrollmentPhase::Idle,
                Some("DP-CC98"),
                vec![pet.clone()],
                Some("abc"),
                false,
                None,
                false,
            )),
            "Securing DP-CC98…"
        );
        assert_eq!(
            tray_tooltip(&snap(
                BlePhase::Disconnected,
                SecureSessionState::Disconnected,
                EnrollmentPhase::Idle,
                None,
                vec![pet],
                Some("abc"),
                false,
                Some("BLE connect timed out"),
                false,
            )),
            "DP-CC98 is offline · BLE connect timed out"
        );
    }
}
