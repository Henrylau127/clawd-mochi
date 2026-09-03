use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, ValueNotification,
    WriteType,
};
#[cfg(target_vendor = "apple")]
use btleplug::platform::PeripheralId;
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::pairing::PairingWindow;
use super::reconnect::Backoff;
use super::roster::{self, FoundPet};
use super::session::{self, Handshake, Notification};
use crate::bridge::protocol::{prepend_session_start_if_needed, DeskPetEvent};
use crate::diagnostics::redact;
use crate::state::{BlePhase, BridgeCommand, EnrollmentPhase, Shared};

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d001);
pub const CMD_RX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d002);
pub const STATUS_TX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d003);
#[allow(dead_code)]
pub const CONFIG_RX_UUID: Uuid = Uuid::from_u128(0x7d3f1b2e_5a8c_4f61_9e09_9a7d4955d004);
const BLE_OPERATION_TIMEOUT: Duration = Duration::from_secs(45);
/// Periodic encrypted status read so CoreBluetooth does not treat the link as idle.
const BLE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
/// First keepalive runs sooner to catch zombie sessions after pet reboot / flash.
const BLE_KEEPALIVE_INITIAL: Duration = Duration::from_secs(5);
const BLE_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(5);

struct Session {
    peripheral: Peripheral,
    cmd_rx: Characteristic,
    status_tx: Characteristic,
    id: String,
    name: String,
    last_activity: Instant,
    keepalive_validated: bool,
}

pub async fn run(shared: Arc<Shared>, mut commands: mpsc::Receiver<BridgeCommand>) {
    if super::mock::mock_enabled() {
        super::mock::run(shared, commands).await;
        return;
    }

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
            shared.set_enrollment(EnrollmentPhase::NoPetFound).await;
            tracing::info!("pairing window closed");
        }
        sync_pairing_status(&shared, &pairing).await;

        // Run outside select! so Cursor hook traffic cannot starve disconnect detection
        // (same failure mode as the reconnect timer bug).
        if let Some(active) = session.as_mut() {
            let link_up = active.peripheral.is_connected().await.unwrap_or(false);
            if !shared.is_connected().await || !link_up {
                tracing::info!("BLE disconnected from {}", active.name);
                let stale = session.take();
                reset_session(&shared, stale).await;
            } else if keepalive_due(active) {
                match verify_firmware_link(active, &shared).await {
                    Ok(()) => {
                        active.last_activity = Instant::now();
                        active.keepalive_validated = true;
                    }
                    Err(err) => {
                        tracing::warn!("BLE keepalive failed: {}", redact(&err));
                        shared.set_link_verified(false).await;
                        shared.set_error(Some(redact(&err))).await;
                        let stale = session.take();
                        reset_session(&shared, stale).await;
                    }
                }
            }
        }

        tokio::select! {
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { break; };
                handle_command(&adapter, shared.clone(), &mut session, &mut pairing, &mut backoff, cmd).await;
            }
            // Do not add an unconditional short sleep here — it always wins over the
            // reconnect timer below and permanently cancels auto-connect.
            _ = tokio::time::sleep(Duration::from_millis(500)), if session.is_none() => {
                attempt_connect(&adapter, shared.clone(), &mut session, &mut pairing, &mut backoff).await;
            }
            _ = tokio::time::sleep(Duration::from_millis(250)), if session.is_some() => {}
        }
    }
}

async fn sync_pairing_status(shared: &Shared, pairing: &PairingWindow) {
    shared.set_pairing_deadline(pairing.deadline()).await;
}

async fn drain_commands(commands: &mut mpsc::Receiver<BridgeCommand>) {
    while commands.recv().await.is_some() {}
}

async fn handle_command(
    adapter: &Adapter,
    shared: Arc<Shared>,
    session: &mut Option<Session>,
    pairing: &mut PairingWindow,
    backoff: &mut Backoff,
    cmd: BridgeCommand,
) {
    match cmd {
        BridgeCommand::Reconnect => {
            let stale = session.take();
            reset_session(&shared, stale).await;
            backoff.reset();
            attempt_connect(adapter, shared, session, pairing, backoff).await;
        }
        BridgeCommand::StartPairing => {
            pairing.start();
            shared.set_pairing(true).await;
            shared
                .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                .await;
            sync_pairing_status(&shared, pairing).await;
        }
        BridgeCommand::StopPairing => {
            pairing.stop();
            shared.set_pairing(false).await;
            shared.set_enrollment(EnrollmentPhase::Idle).await;
        }
        BridgeCommand::Unbond => {
            pairing.stop();
            shared.set_pairing(false).await;
            shared.set_enrollment(EnrollmentPhase::Idle).await;
            send_unbond(&shared, session).await;
        }
        BridgeCommand::AddPet => {
            pairing.start();
            shared.set_pairing(true).await;
            shared
                .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                .await;
            let stale = session.take();
            reset_session(&shared, stale).await;
            sync_pairing_status(&shared, pairing).await;
            backoff.reset();
            tokio::time::sleep(Duration::from_secs(1)).await;
            attempt_connect(adapter, shared, session, pairing, backoff).await;
        }
        BridgeCommand::Send(event) => {
            if event.event == "pairingStart" {
                pairing.start();
                shared.set_pairing(true).await;
                shared
                    .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                    .await;
                sync_pairing_status(&shared, pairing).await;
                return;
            }
            if event.event == "pairingStop" || event.event == "unbond" {
                pairing.stop();
                shared.set_pairing(false).await;
                shared.set_enrollment(EnrollmentPhase::Idle).await;
                if event.event == "unbond" {
                    send_unbond(&shared, session).await;
                }
                return;
            }
            send_or_queue(&shared, session, event).await;
        }
    }
}

async fn send_or_queue(shared: &Shared, session: &mut Option<Session>, event: DeskPetEvent) {
    shared.record_event(&event.event).await;
    if let Some(active) = session.as_mut() {
        let pet_mode = shared.pet_mode().await;
        let events = prepend_session_start_if_needed(vec![event.clone()], pet_mode.as_deref());
        for (index, pending) in events.iter().enumerate() {
            if let Err(err) = write_event(active, pending).await {
                tracing::warn!("BLE write failed: {}", redact(&err));
                shared.set_error(Some(redact(&err))).await;
                for rest in &events[index..] {
                    shared.enqueue(rest.clone()).await;
                }
                let stale = session.take();
                reset_session(shared, stale).await;
                return;
            }
        }
        if !shared.is_link_verified().await {
            if let Err(err) = verify_firmware_link(active, shared).await {
                tracing::warn!("BLE link verification failed after write: {}", redact(&err));
                shared.set_link_verified(false).await;
                shared.set_error(Some(redact(&err))).await;
                let stale = session.take();
                reset_session(shared, stale).await;
                return;
            }
            active.keepalive_validated = true;
        }
        active.last_activity = Instant::now();
        shared.set_error(None).await;
    } else {
        shared.enqueue(event).await;
    }
}

async fn send_unbond(shared: &Shared, session: &mut Option<Session>) {
    let Some(active) = session.take() else {
        shared.reset_secure_session(None).await;
        return;
    };

    let mut event = DeskPetEvent::new("unbond");
    event.ensure_id();
    if let Err(err) = write_event(&active, &event).await {
        shared.set_error(Some(redact(&err))).await;
    }
    let id = active.id.clone();
    let _ = active.peripheral.disconnect().await;
    shared.reset_secure_session(Some(&id)).await;
}

async fn reset_session(shared: &Shared, stale: Option<Session>) {
    if let Some(active) = stale {
        let _ = active.peripheral.disconnect().await;
    }
    // Always clear UI state on teardown — the session_id guard caused stale
    // "connected" when the status listener had already cleared the id.
    shared.reset_secure_session(None).await;
}

async fn write_event(session: &Session, event: &DeskPetEvent) -> Result<(), String> {
    let bytes = event.to_ble_bytes().map_err(|err| err.to_string())?;
    tokio::time::timeout(
        BLE_OPERATION_TIMEOUT,
        session
            .peripheral
            .write(&session.cmd_rx, &bytes, WriteType::WithResponse),
    )
    .await
    .map_err(|_| {
        format!(
            "BLE operation timed out after {}s; pairing or secure link may be required",
            BLE_OPERATION_TIMEOUT.as_secs()
        )
    })?
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn keepalive_due(session: &Session) -> bool {
    let interval = if session.keepalive_validated {
        BLE_KEEPALIVE_INTERVAL
    } else {
        BLE_KEEPALIVE_INITIAL
    };
    session.last_activity.elapsed() >= interval
}

async fn verify_firmware_link(session: &Session, shared: &Shared) -> Result<(), String> {
    let bytes = tokio::time::timeout(BLE_KEEPALIVE_TIMEOUT, session.peripheral.read(&session.status_tx))
        .await
        .map_err(|_| format!("BLE keepalive timed out after {}s", BLE_KEEPALIVE_TIMEOUT.as_secs()))?
        .map_err(|err| err.to_string())?;
    let status = session::parse_status_read(&bytes)
        .map_err(|err| format!("BLE status read invalid: {err}"))?;
    if !status.secure_session_confirmed {
        return Err(format!(
            "firmware reports link down (ble_connected={})",
            status.ble_connected
        ));
    }
    shared
        .set_pet_status_for_session(&session.id, status.mode.clone(), status.state.clone())
        .await;
    shared.set_link_verified(true).await;
    Ok(())
}

async fn attempt_connect(
    adapter: &Adapter,
    shared: Arc<Shared>,
    session: &mut Option<Session>,
    pairing: &mut PairingWindow,
    backoff: &mut Backoff,
) {
    if session.is_some() {
        return;
    }
    shared.set_phase(BlePhase::Scanning, None).await;
    if pairing.is_active() {
        shared.set_enrollment(EnrollmentPhase::Scanning).await;
    }
    match connect_once(adapter, shared.clone(), pairing.is_active()).await {
        Ok(Some(new_session)) => {
            backoff.reset();
            *session = Some(new_session);
            let flushed = match session.as_ref() {
                Some(active) => flush_queue(&shared, active).await,
                None => true,
            };
            if !flushed {
                let stale = session.take();
                reset_session(&shared, stale).await;
                return;
            }
            let active = session.as_ref().expect("session set after flush");
            if let Err(err) = verify_firmware_link(active, &shared).await {
                tracing::warn!("BLE link verification failed: {}", redact(&err));
                shared.set_error(Some(redact(&err))).await;
                let stale = session.take();
                reset_session(&shared, stale).await;
                return;
            }
            let pet_id = active.id.clone();
            let pet_name = active.name.clone();
            tracing::info!("connected to {}", pet_name);
            shared
                .mark_secure_session_ready(&pet_id, &pet_name)
                .await;
            if let Some(active) = session.as_mut() {
                active.keepalive_validated = true;
            }
            shared
                .remember_pet(pet_id, pet_name)
                .await;
            if pairing.is_active() {
                pairing.stop();
                shared.set_pairing(false).await;
            }
            shared.set_error(None).await;
        }
        Ok(None) => {
            shared.set_phase(BlePhase::Disconnected, None).await;
            if pairing.is_active() {
                shared.set_enrollment(EnrollmentPhase::NoPetFound).await;
            }
            tokio::time::sleep(backoff.next()).await;
        }
        Err(err) => {
            tracing::warn!("BLE connect failed: {}", redact(&err));
            shared.set_error(Some(redact(&err))).await;
            shared.set_phase(BlePhase::Disconnected, None).await;
            if pairing.is_active() {
                shared.set_enrollment(EnrollmentPhase::NoPetFound).await;
            }
            tokio::time::sleep(backoff.next()).await;
        }
    }
}

async fn flush_queue(shared: &Shared, session: &Session) -> bool {
    let pet_mode = shared.pet_mode().await;
    let events = prepend_session_start_if_needed(shared.drain_queue().await, pet_mode.as_deref());
    for (index, event) in events.iter().enumerate() {
        if let Err(err) = write_event(session, &event).await {
            tracing::warn!("queued write failed: {}", redact(&err));
            shared.set_error(Some(redact(&err))).await;
            for pending in &events[index..] {
                shared.enqueue(pending.clone()).await;
            }
            return false;
        }
    }
    true
}

async fn connect_once(
    adapter: &Adapter,
    shared: Arc<Shared>,
    pairing: bool,
) -> Result<Option<Session>, String> {
    let roster = shared.roster_snapshot().await;
    if !pairing {
        let active_name = roster.active_pet_id.as_deref().and_then(|active_id| {
            roster
                .pets
                .iter()
                .find(|pet| pet.id == active_id || pet.name == active_id)
                .map(|pet| pet.name.as_str())
        });
        if let Some(candidate) =
            cached_candidate(adapter, roster.active_pet_id.as_deref(), active_name).await
        {
            match connect_candidate(candidate, shared.clone(), false).await {
                Ok(session) => return Ok(Some(session)),
                Err(err) => {
                    tracing::debug!("cached BLE peripheral unavailable: {}", redact(&err));
                    shared.reset_secure_session(None).await;
                }
            }
        }
    }

    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|err| err.to_string())?;
    tokio::time::sleep(Duration::from_secs(4)).await;
    let _ = adapter.stop_scan().await;

    let candidates = discover_desk_pets(adapter).await?;
    let known_ids: Vec<String> = roster
        .pets
        .iter()
        .flat_map(|pet| [pet.id.clone(), pet.name.clone()])
        .collect();
    let found: Vec<FoundPet> = candidates
        .iter()
        .map(|candidate| FoundPet {
            id: candidate.id.clone(),
            name: candidate.name.clone(),
        })
        .collect();
    let Some(chosen) =
        roster::choose_pet(&found, roster.active_pet_id.as_deref(), &known_ids, pairing)
    else {
        return Ok(None);
    };
    let Some(candidate) = candidates.into_iter().find(|item| item.id == chosen.id) else {
        return Ok(None);
    };

    connect_candidate(candidate, shared, pairing)
        .await
        .map(Some)
        .map_err(|err| err.to_string())
}

async fn connect_candidate(
    candidate: Candidate,
    shared: Arc<Shared>,
    enrolling: bool,
) -> Result<Session, String> {
    shared
        .begin_secure_session(&candidate.id, &candidate.name, enrolling)
        .await;
    candidate
        .peripheral
        .connect()
        .await
        .map_err(|err| err.to_string())?;
    let result = establish_session(&candidate, shared).await;
    if result.is_err() {
        let _ = candidate.peripheral.disconnect().await;
    }
    result
}

async fn establish_session(candidate: &Candidate, shared: Arc<Shared>) -> Result<Session, String> {
    candidate
        .peripheral
        .discover_services()
        .await
        .map_err(|err| err.to_string())?;

    let chars = candidate.peripheral.characteristics();
    let cmd_rx = chars
        .iter()
        .find(|c| c.uuid == CMD_RX_UUID)
        .cloned()
        .ok_or_else(|| "command characteristic missing".to_string())?;
    let status_tx = chars
        .iter()
        .find(|c| c.uuid == STATUS_TX_UUID)
        .cloned()
        .ok_or_else(|| "status characteristic missing".to_string())?;
    let status_bytes =
        tokio::time::timeout(BLE_OPERATION_TIMEOUT, candidate.peripheral.read(&status_tx))
            .await
            .map_err(|_| {
                format!(
                    "BLE security handshake timed out after {}s; enter the pet passkey if prompted",
                    BLE_OPERATION_TIMEOUT.as_secs()
                )
            })?
            .map_err(|err| format!("BLE security handshake failed: {err}"))?;
    let mut handshake = Handshake::new();
    let status = handshake
        .record_encrypted_status_read(&status_bytes)
        .map_err(|err| format!("BLE firmware confirmation failed: {err}"))?;

    let notifications =
        tokio::time::timeout(BLE_OPERATION_TIMEOUT, candidate.peripheral.notifications())
            .await
            .map_err(|_| {
                format!(
                    "status notification setup timed out after {}s",
                    BLE_OPERATION_TIMEOUT.as_secs()
                )
            })?
            .map_err(|err| format!("status notification setup failed: {err}"))?;
    tokio::time::timeout(
        BLE_OPERATION_TIMEOUT,
        candidate.peripheral.subscribe(&status_tx),
    )
    .await
    .map_err(|_| {
        format!(
            "status subscription timed out after {}s",
            BLE_OPERATION_TIMEOUT.as_secs()
        )
    })?
    .map_err(|err| format!("status subscription failed: {err}"))?;
    handshake
        .record_status_subscription()
        .map_err(|err| format!("BLE secure session incomplete: {err}"))?;
    handshake
        .ensure_ready()
        .map_err(|err| format!("BLE secure session incomplete: {err}"))?;
    shared
        .set_pet_status_for_session(&candidate.id, status.mode.clone(), status.state.clone())
        .await;
    spawn_status_listener(notifications, shared.clone(), candidate.id.clone());

    Ok(Session {
        peripheral: candidate.peripheral.clone(),
        cmd_rx,
        status_tx,
        id: candidate.id.clone(),
        name: candidate.name.clone(),
        last_activity: Instant::now(),
        keepalive_validated: false,
    })
}

struct Candidate {
    peripheral: Peripheral,
    id: String,
    name: String,
}

#[cfg(target_vendor = "apple")]
async fn cached_candidate(
    adapter: &Adapter,
    active_id: Option<&str>,
    active_name: Option<&str>,
) -> Option<Candidate> {
    let uuid = Uuid::parse_str(active_id?).ok()?;
    let peripheral = adapter.peripheral(&PeripheralId::from(uuid)).await.ok()?;
    let name = peripheral
        .properties()
        .await
        .ok()
        .flatten()
        .and_then(|props| {
            let name_ok = props.local_name.as_deref().is_some_and(|name| {
                name.starts_with("DP-") && name != "DP-????" && name != "DP-xxxx"
            });
            let service_ok = props.services.contains(&SERVICE_UUID);
            (name_ok || service_ok).then(|| roster::display_name(props.local_name))
        })
        .or_else(|| active_name.map(str::to_string))?;
    Some(Candidate {
        id: peripheral.id().to_string(),
        name,
        peripheral,
    })
}

#[cfg(not(target_vendor = "apple"))]
async fn cached_candidate(
    _adapter: &Adapter,
    _active_id: Option<&str>,
    _active_name: Option<&str>,
) -> Option<Candidate> {
    None
}

async fn discover_desk_pets(adapter: &Adapter) -> Result<Vec<Candidate>, String> {
    let peripherals = adapter.peripherals().await.map_err(|err| err.to_string())?;
    let mut found = Vec::new();
    for peripheral in peripherals {
        let Some(props) = peripheral.properties().await.ok().flatten() else {
            continue;
        };
        let name_ok = props
            .local_name
            .as_deref()
            .is_some_and(|name| name.starts_with("DP-") && name != "DP-????" && name != "DP-xxxx");
        let service_ok = props.services.contains(&SERVICE_UUID);
        if !(name_ok || service_ok) {
            continue;
        }
        found.push(Candidate {
            id: peripheral.id().to_string(),
            name: roster::display_name(props.local_name),
            peripheral,
        });
    }
    Ok(found)
}

fn spawn_status_listener(
    mut notifications: Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>,
    shared: Arc<Shared>,
    session_id: String,
) {
    tokio::spawn(async move {
        while let Some(notification) = notifications.next().await {
            if notification.uuid != STATUS_TX_UUID {
                continue;
            }
            match session::parse_notification(&notification.value) {
                Ok(Notification::FirmwareStatus(status)) => {
                    shared
                        .set_pet_status_for_session(
                            &session_id,
                            status.mode.clone(),
                            status.state.clone(),
                        )
                        .await;
                    if status.secure_session_confirmed {
                        shared.set_link_verified(true).await;
                    }
                }
                Ok(Notification::Ack(ack)) => {
                    shared
                        .set_pet_status_for_session(&session_id, ack.mode, ack.state)
                        .await;
                    if ack.ok {
                        shared.set_link_verified(true).await;
                    }
                    if !ack.ok {
                        shared.set_error(ack.error.as_deref().map(redact)).await;
                    }
                }
                Err(session::SessionError::FirmwareSessionNotReady) => {
                    tracing::warn!("firmware secure session is no longer ready");
                    shared
                        .set_error(Some(
                            "firmware secure session is no longer ready".to_string(),
                        ))
                        .await;
                    break;
                }
                Err(err) => {
                    let message = redact(&err.to_string());
                    tracing::warn!("invalid BLE status notification: {}", message);
                    shared.set_error(Some(message)).await;
                    break;
                }
            }
        }
        shared.reset_secure_session(None).await;
    });
}

#[cfg(test)]
mod tests {
    use futures::stream;

    use super::*;
    use crate::state::SecureSessionState;

    #[tokio::test]
    async fn invalid_status_notification_terminates_the_secure_session() {
        let (tx, _rx) = mpsc::channel(8);
        let shared = Arc::new(Shared::new(tx));
        shared
            .begin_secure_session("test-id", "DP-TEST", false)
            .await;
        shared.mark_secure_session_ready("test-id", "DP-TEST").await;

        let notifications = stream::iter(vec![
            ValueNotification {
                uuid: STATUS_TX_UUID,
                value: b"not json".to_vec(),
            },
            ValueNotification {
                uuid: STATUS_TX_UUID,
                value: br#"{"mode":"agent","state":"thinking","ble_connected":true}"#.to_vec(),
            },
        ])
        .chain(stream::pending());
        spawn_status_listener(
            Box::pin(notifications),
            shared.clone(),
            "test-id".to_string(),
        );

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if shared.snapshot().await.secure_session == SecureSessionState::Disconnected {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("invalid status notification should reset the session");
    }
}
