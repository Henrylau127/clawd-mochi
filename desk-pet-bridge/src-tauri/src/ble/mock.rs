use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::pairing::PairingWindow;
use crate::bridge::protocol::DeskPetEvent;
use crate::state::{BlePhase, BridgeCommand, EnrollmentPhase, Shared};

pub const MOCK_DEVICE_NAME: &str = "DP-TEST";

pub fn mock_enabled() -> bool {
    matches!(
        std::env::var("DESKPET_BLE_MOCK").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

/// In-memory stand-in for firmware `JsonProtocol` + `PetStateMachine`.
#[derive(Debug, Clone)]
pub struct SimulatedPet {
    pub mode: String,
    pub state: String,
    pub pairing_active: bool,
    pub last_ack: Value,
}

impl Default for SimulatedPet {
    fn default() -> Self {
        Self {
            mode: "wander".to_string(),
            state: "idle".to_string(),
            pairing_active: false,
            last_ack: json!({ "ok": true, "mode": "wander", "state": "idle" }),
        }
    }
}

impl SimulatedPet {
    pub fn apply(&mut self, event: &DeskPetEvent) -> Value {
        let ok = match event.event.as_str() {
            "sessionStart" => {
                self.mode = "agent".to_string();
                self.state = event
                    .state
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "coding".to_string());
                true
            }
            "sessionEnd" => {
                self.mode = "wander".to_string();
                self.state = "idle".to_string();
                true
            }
            "stateUpdate" => {
                if self.mode == "agent" {
                    if let Some(state) = &event.state {
                        self.state = state.clone();
                    }
                }
                true
            }
            "sleep" => {
                self.mode = "sleep".to_string();
                true
            }
            "wake" => {
                if self.mode == "sleep" {
                    self.mode = "wander".to_string();
                    self.state = "idle".to_string();
                }
                true
            }
            "pairingStart" => {
                self.pairing_active = true;
                true
            }
            "pairingStop" | "unbond" => {
                self.pairing_active = false;
                true
            }
            "attentionRequired" | "attentionCleared" | "securityEvent" | "quotaUpdate" => true,
            _ => false,
        };

        let mut ack = json!({
            "ok": ok,
            "mode": self.mode,
            "state": self.state,
        });
        if let Some(id) = &event.id {
            ack["id"] = json!(id);
        }
        if !ok {
            ack["error"] = json!("invalid transition");
        }
        self.last_ack = ack.clone();
        ack
    }
}

pub async fn run(shared: Arc<Shared>, mut commands: mpsc::Receiver<BridgeCommand>) {
    tracing::info!("BLE mock enabled; simulated peripheral {MOCK_DEVICE_NAME}");
    shared
        .mark_secure_session_ready("mock", MOCK_DEVICE_NAME)
        .await;
    shared.set_simulated_pet("wander", "idle").await;

    let mut pet = SimulatedPet::default();
    let mut pairing = PairingWindow::new();

    loop {
        if pairing.expire_if_needed() {
            pet.pairing_active = false;
            shared.set_pairing(false).await;
            shared.set_enrollment(EnrollmentPhase::NoPetFound).await;
        }
        sync_pairing_status(&shared, &pairing).await;

        tokio::select! {
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { break; };
                handle_command(&shared, &mut pet, &mut pairing, cmd).await;
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

async fn sync_pairing_status(shared: &Shared, pairing: &PairingWindow) {
    shared.set_pairing_deadline(pairing.deadline()).await;
}

async fn handle_command(
    shared: &Shared,
    pet: &mut SimulatedPet,
    pairing: &mut PairingWindow,
    cmd: BridgeCommand,
) {
    match cmd {
        BridgeCommand::Reconnect => {
            shared.set_phase(BlePhase::Scanning, None).await;
            tokio::time::sleep(Duration::from_millis(30)).await;
            shared
                .begin_secure_session("mock", MOCK_DEVICE_NAME, false)
                .await;
            shared
                .mark_secure_session_ready("mock", MOCK_DEVICE_NAME)
                .await;
            shared
                .set_pet_status(Some(pet.mode.clone()), Some(pet.state.clone()))
                .await;
            flush_queue(shared, pet).await;
        }
        BridgeCommand::StartPairing => {
            pairing.start();
            shared
                .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                .await;
            sync_pairing_status(shared, pairing).await;
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
            shared.reset_secure_session(None).await;
        }
        BridgeCommand::AddPet => {
            pairing.start();
            shared.set_pairing(true).await;
            sync_pairing_status(shared, pairing).await;
            shared
                .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                .await;
            shared.set_phase(BlePhase::Scanning, None).await;
            tokio::time::sleep(Duration::from_millis(30)).await;
            shared
                .begin_secure_session("mock", MOCK_DEVICE_NAME, true)
                .await;
            shared
                .mark_secure_session_ready("mock", MOCK_DEVICE_NAME)
                .await;
            shared
                .remember_pet("mock".to_string(), MOCK_DEVICE_NAME.to_string())
                .await;
            pairing.stop();
            shared.set_pairing(false).await;
        }
        BridgeCommand::Send(event) => {
            if event.event == "pairingStart" {
                pairing.start();
                shared.set_pairing(true).await;
                shared
                    .set_enrollment(EnrollmentPhase::AwaitingDevicePair)
                    .await;
                sync_pairing_status(shared, pairing).await;
                return;
            }
            if event.event == "pairingStop" || event.event == "unbond" {
                pairing.stop();
                shared.set_pairing(false).await;
                shared.set_enrollment(EnrollmentPhase::Idle).await;
                if event.event == "unbond" {
                    shared.reset_secure_session(None).await;
                }
                return;
            }
            apply_event(shared, pet, pairing, event).await;
        }
    }
}

async fn apply_event(
    shared: &Shared,
    pet: &mut SimulatedPet,
    pairing: &mut PairingWindow,
    event: DeskPetEvent,
) {
    if event.event == "pairingStart" {
        pairing.start();
        shared.set_pairing(true).await;
        sync_pairing_status(shared, pairing).await;
    }
    if event.event == "pairingStop" || event.event == "unbond" {
        pairing.stop();
        shared.set_pairing(false).await;
    }
    shared.record_event(&event.event).await;
    pet.apply(&event);
    shared.set_simulated_pet(&pet.mode, &pet.state).await;
    shared.set_error(None).await;
}

async fn flush_queue(shared: &Shared, pet: &mut SimulatedPet) {
    let mut pairing = PairingWindow::new();
    for event in shared.drain_queue().await {
        apply_event(shared, pet, &mut pairing, event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_start_mirrors_firmware_agent_mode() {
        let mut pet = SimulatedPet::default();
        let mut event = DeskPetEvent::new("sessionStart").with_state("coding");
        event.ensure_id();
        let ack = pet.apply(&event);
        assert_eq!(ack["ok"], true);
        assert_eq!(ack["mode"], "agent");
        assert_eq!(ack["state"], "coding");
        assert_eq!(pet.mode, "agent");
    }

    #[test]
    fn wake_ignored_when_not_sleeping() {
        let mut pet = SimulatedPet::default();
        let ack = pet.apply(&DeskPetEvent::new("wake"));
        assert_eq!(ack["ok"], true);
        assert_eq!(pet.mode, "wander");
    }

    #[test]
    fn sleep_then_wake() {
        let mut pet = SimulatedPet::default();
        pet.apply(&DeskPetEvent::new("sleep"));
        assert_eq!(pet.mode, "sleep");
        pet.apply(&DeskPetEvent::new("wake"));
        assert_eq!(pet.mode, "wander");
        assert_eq!(pet.state, "idle");
    }
}
