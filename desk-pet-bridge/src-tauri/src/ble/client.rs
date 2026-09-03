use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::pairing::PairingWindow;
use super::reconnect::Backoff;
use crate::bridge::protocol::DeskPetEvent;
use crate::diagnostics::redact;
use crate::state::{BlePhase, BridgeCommand, Shared};

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d001);
pub const CMD_RX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d002);
pub const STATUS_TX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d003);
pub const CONFIG_RX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d004);

struct Session {
    peripheral: Peripheral,
    cmd_rx: Characteristic,
    name: String,
}

pub async fn run(shared: Arc<Shared>, mut commands: mpsc::Receiver<BridgeCommand>) {
    let manager = match Manager::new().await {
        Ok(manager) => manager,
        Err(err) => {
            tracing::error!("BLE manager unavailable: {}", redact(&err.to_string()));
            drain_commands(&mut commands).await;
            return;
        }
    };

    let adapter = match manager.adapters().await {
        Ok(mut adapters) => adapters.pop(),
        Err(err) => {
            tracing::error!("BLE adapters unavailable: {}", redact(&err.to_string()));
            None
        }
    };

    let Some(adapter) = adapter else {
        tracing::error!("no BLE adapter; loopback API will queue events");
        drain_commands(&mut commands).await;
        return;
    };

    let mut session: Option<Session> = None;
    let mut backoff = Backoff::new();
    let mut pairing = PairingWindow::new();

    loop {
        if pairing.expire_if_needed() {
            shared.set_pairing(false).await;
            tracing::info!("pairing window closed");
        }

        tokio::select! {
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { break; };
                handle_command(&adapter, &shared, &mut session, &mut pairing, &mut backoff, cmd).await;
            }
            _ = tokio::time::sleep(Duration::from_millis(500)), if session.is_none() => {
                attempt_connect(&adapter, &shared, &mut session, &mut backoff).await;
            }
            _ = tokio::time::sleep(Duration::from_secs(2)), if session.is_some() => {
                if let Some(active) = session.as_ref() {
                    if !active.peripheral.is_connected().await.unwrap_or(false) {
                        tracing::info!("BLE disconnected from {}", active.name);
                        session = None;
                        shared.set_phase(BlePhase::Disconnected, None).await;
                    }
                }
            }
        }
    }
}

async fn drain_commands(commands: &mut mpsc::Receiver<BridgeCommand>) {
    while commands.recv().await.is_some() {}
}

async fn handle_command(
    adapter: &Adapter,
    shared: &Shared,
    session: &mut Option<Session>,
    pairing: &mut PairingWindow,
    backoff: &mut Backoff,
    cmd: BridgeCommand,
) {
    match cmd {
        BridgeCommand::Reconnect => {
            if let Some(active) = session.take() {
                let _ = active.peripheral.disconnect().await;
            }
            shared.set_phase(BlePhase::Disconnected, None).await;
            backoff.reset();
            attempt_connect(adapter, shared, session, backoff).await;
        }
        BridgeCommand::StartPairing => {
            pairing.start();
            shared.set_pairing(true).await;
            let mut event = DeskPetEvent::new("pairingStart");
            event.ensure_id();
            send_or_queue(shared, session, event).await;
        }
        BridgeCommand::StopPairing => {
            pairing.stop();
            shared.set_pairing(false).await;
            let mut event = DeskPetEvent::new("pairingStop");
            event.ensure_id();
            send_or_queue(shared, session, event).await;
        }
        BridgeCommand::Unbond => {
            pairing.stop();
            shared.set_pairing(false).await;
            let mut event = DeskPetEvent::new("unbond");
            event.ensure_id();
            send_or_queue(shared, session, event).await;
        }
        BridgeCommand::Send(event) => {
            if event.event == "pairingStart" {
                pairing.start();
                shared.set_pairing(true).await;
            }
            if event.event == "pairingStop" || event.event == "unbond" {
                pairing.stop();
                shared.set_pairing(false).await;
            }
            send_or_queue(shared, session, event).await;
        }
    }
}

async fn send_or_queue(shared: &Shared, session: &mut Option<Session>, event: DeskPetEvent) {
    shared.record_event(&event.event).await;
    match session.as_ref() {
        Some(active) => {
            if let Err(err) = write_event(active, &event).await {
                tracing::warn!("BLE write failed: {}", redact(&err));
                shared.set_error(Some(redact(&err))).await;
                shared.enqueue(event).await;
            } else {
                shared.set_error(None).await;
            }
        }
        None => {
            shared.enqueue(event).await;
        }
    }
}

async fn write_event(session: &Session, event: &DeskPetEvent) -> Result<(), String> {
    let bytes = event.to_ble_bytes().map_err(|err| err.to_string())?;
    session
        .peripheral
        .write(&session.cmd_rx, &bytes, WriteType::WithResponse)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

async fn attempt_connect(
    adapter: &Adapter,
    shared: &Shared,
    session: &mut Option<Session>,
    backoff: &mut Backoff,
) {
    if session.is_some() {
        return;
    }
    shared.set_phase(BlePhase::Scanning, None).await;
    match connect_once(adapter).await {
        Ok(Some(new_session)) => {
            tracing::info!("connected to {}", new_session.name);
            shared
                .set_phase(BlePhase::Connected, Some(new_session.name.clone()))
                .await;
            shared.set_error(None).await;
            backoff.reset();
            flush_queue(shared, &new_session).await;
            *session = Some(new_session);
        }
        Ok(None) => {
            shared.set_phase(BlePhase::Disconnected, None).await;
            tokio::time::sleep(backoff.next()).await;
        }
        Err(err) => {
            tracing::warn!("BLE connect failed: {}", redact(&err));
            shared.set_error(Some(redact(&err))).await;
            shared.set_phase(BlePhase::Disconnected, None).await;
            tokio::time::sleep(backoff.next()).await;
        }
    }
}

async fn flush_queue(shared: &Shared, session: &Session) {
    for event in shared.drain_queue().await {
        if let Err(err) = write_event(session, &event).await {
            tracing::warn!("queued write failed: {}", redact(&err));
            shared.enqueue(event).await;
            break;
        }
    }
}

async fn connect_once(adapter: &Adapter) -> Result<Option<Session>, String> {
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|err| err.to_string())?;
    tokio::time::sleep(Duration::from_secs(4)).await;
    let _ = adapter.stop_scan().await;

    let Some(peripheral) = find_desk_pet(adapter).await? else {
        return Ok(None);
    };

    let name = peripheral
        .properties()
        .await
        .ok()
        .flatten()
        .and_then(|props| props.local_name)
        .unwrap_or_else(|| "DP-????".to_string());

    peripheral.connect().await.map_err(|err| err.to_string())?;
    peripheral
        .discover_services()
        .await
        .map_err(|err| err.to_string())?;

    let chars = peripheral.characteristics();
    let cmd_rx = chars
        .iter()
        .find(|c| c.uuid == CMD_RX_UUID)
        .cloned()
        .ok_or_else(|| "command characteristic missing".to_string())?;
    if let Some(status_tx) = chars.iter().find(|c| c.uuid == STATUS_TX_UUID).cloned() {
        if let Err(err) = peripheral.subscribe(&status_tx).await {
            tracing::warn!("status subscribe failed: {}", redact(&err.to_string()));
        } else {
            spawn_ack_logger(peripheral.clone());
        }
    }
    let _ = chars.iter().find(|c| c.uuid == CONFIG_RX_UUID);

    Ok(Some(Session {
        peripheral,
        cmd_rx,
        name,
    }))
}

async fn find_desk_pet(adapter: &Adapter) -> Result<Option<Peripheral>, String> {
    let peripherals = adapter.peripherals().await.map_err(|err| err.to_string())?;
    for peripheral in peripherals {
        let Some(props) = peripheral.properties().await.ok().flatten() else {
            continue;
        };
        let name_ok = props
            .local_name
            .as_deref()
            .is_some_and(|name| name.starts_with("DP-"));
        let service_ok = props.services.contains(&SERVICE_UUID);
        if name_ok || service_ok {
            return Ok(Some(peripheral));
        }
    }
    Ok(None)
}

fn spawn_ack_logger(peripheral: Peripheral) {
    tokio::spawn(async move {
        let Ok(mut stream) = peripheral.notifications().await else {
            return;
        };
        while let Some(notification) = stream.next().await {
            if notification.uuid != STATUS_TX_UUID {
                continue;
            }
            if let Ok(text) = std::str::from_utf8(&notification.value) {
                tracing::info!("status notify {}", redact(text));
            }
        }
    });
}
