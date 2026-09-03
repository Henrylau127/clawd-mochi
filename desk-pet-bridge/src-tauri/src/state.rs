use serde::Serialize;
use std::time::Instant;
use tokio::sync::{mpsc, watch, Mutex};

use crate::bridge::loopback::{self, DEFAULT_PORT};
use crate::bridge::protocol::DeskPetEvent;
use crate::bridge::queue::EventQueue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlePhase {
    Disconnected,
    Scanning,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureSessionState {
    Disconnected,
    Handshaking,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentPhase {
    Idle,
    AwaitingDevicePair,
    Scanning,
    PasskeyPending,
    NoPetFound,
    Success,
}

#[derive(Debug, Clone)]
pub enum BridgeCommand {
    Reconnect,
    StartPairing,
    StopPairing,
    Unbond,
    AddPet,
    Send(DeskPetEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusSnapshot {
    pub ble: BlePhase,
    pub secure_session: SecureSessionState,
    pub enrollment: EnrollmentPhase,
    pub device: Option<String>,
    pub queue: usize,
    pub queue_items: Vec<DeskPetEvent>,
    pub pairing_active: bool,
    pub pairing_remaining_seconds: Option<u64>,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pet_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pet_state: Option<String>,
    pub loopback_host: String,
    pub loopback_port: u16,
    pub loopback_addr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loopback_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loopback_note: Option<String>,
    pub provider: String,
    pub pets: Vec<loopback::KnownPet>,
    pub active_pet_id: Option<String>,
    pub link_verified: bool,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicStatusSnapshot {
    pub ble: BlePhase,
    pub secure_session: SecureSessionState,
    pub enrollment: EnrollmentPhase,
    pub device: Option<String>,
    pub queue: usize,
    pub pairing_active: bool,
    pub pairing_remaining_seconds: Option<u64>,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pet_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pet_state: Option<String>,
    pub loopback_host: String,
    pub loopback_port: u16,
    pub loopback_addr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loopback_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loopback_note: Option<String>,
    pub provider: String,
    pub pets: Vec<loopback::KnownPet>,
    pub active_pet_id: Option<String>,
    pub link_verified: bool,
    pub ts: u64,
}

struct LoopbackStatus {
    addr: Option<String>,
    port: u16,
    error: Option<String>,
    note: Option<String>,
}

struct BridgeInner {
    ble: BlePhase,
    secure_session: SecureSessionState,
    enrollment: EnrollmentPhase,
    device: Option<String>,
    session_id: Option<String>,
    pairing_active: bool,
    pairing_deadline: Option<Instant>,
    last_event: Option<String>,
    last_error: Option<String>,
    simulated_mode: Option<String>,
    simulated_state: Option<String>,
    link_verified: bool,
}

pub struct Shared {
    inner: Mutex<BridgeInner>,
    outgoing: Mutex<EventQueue>,
    loopback: Mutex<LoopbackStatus>,
    provider: Mutex<String>,
    roster: Mutex<loopback::PetRoster>,
    port_tx: watch::Sender<u16>,
    _port_rx: watch::Receiver<u16>,
    pub command_tx: mpsc::Sender<BridgeCommand>,
}

impl Shared {
    pub fn new(command_tx: mpsc::Sender<BridgeCommand>) -> Self {
        let port = loopback::resolve_port();
        let (port_tx, port_rx) = watch::channel(port);
        Self {
            inner: Mutex::new(BridgeInner {
                ble: BlePhase::Disconnected,
                secure_session: SecureSessionState::Disconnected,
                enrollment: EnrollmentPhase::Idle,
                device: None,
                session_id: None,
                pairing_active: false,
                pairing_deadline: None,
                last_event: None,
                last_error: None,
                simulated_mode: None,
                simulated_state: None,
                link_verified: false,
            }),
            outgoing: Mutex::new(EventQueue::default()),
            loopback: Mutex::new(LoopbackStatus {
                addr: None,
                port,
                error: None,
                note: None,
            }),
            provider: Mutex::new(loopback::load_saved_provider()),
            roster: Mutex::new(if cfg!(test) {
                loopback::PetRoster::default()
            } else {
                loopback::load_roster()
            }),
            port_tx,
            _port_rx: port_rx,
            command_tx,
        }
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let inner = self.inner.lock().await;
        let outgoing = self.outgoing.lock().await;
        let queue_items = outgoing.peek();
        let queue = queue_items.len();
        drop(outgoing);
        let loopback = self.loopback.lock().await;
        let port = loopback.port;
        let provider = self.provider.lock().await.clone();
        let roster = self.roster.lock().await.clone();
        let pairing_remaining_seconds = inner.pairing_deadline.and_then(|deadline| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
            (seconds > 0).then_some(seconds)
        });
        StatusSnapshot {
            ble: inner.ble,
            secure_session: inner.secure_session,
            enrollment: inner.enrollment,
            device: inner.device.clone(),
            queue,
            queue_items,
            pairing_active: pairing_remaining_seconds.is_some(),
            pairing_remaining_seconds,
            last_event: inner.last_event.clone(),
            last_error: inner.last_error.clone(),
            pet_mode: inner.simulated_mode.clone(),
            pet_state: inner.simulated_state.clone(),
            loopback_host: loopback::HOST.to_string(),
            loopback_port: port,
            loopback_addr: loopback
                .addr
                .clone()
                .unwrap_or_else(|| format!("{}:{port}", loopback::HOST)),
            loopback_error: loopback.error.clone(),
            loopback_note: loopback.note.clone(),
            provider,
            pets: roster.pets,
            active_pet_id: roster.active_pet_id,
            link_verified: inner.link_verified,
            ts: unix_secs(),
        }
    }

    pub async fn public_snapshot(&self) -> PublicStatusSnapshot {
        let snapshot = self.snapshot().await;
        PublicStatusSnapshot {
            ble: snapshot.ble,
            secure_session: snapshot.secure_session,
            enrollment: snapshot.enrollment,
            device: snapshot.device,
            queue: snapshot.queue,
            pairing_active: snapshot.pairing_active,
            pairing_remaining_seconds: snapshot.pairing_remaining_seconds,
            last_event: snapshot.last_event,
            last_error: snapshot.last_error,
            pet_mode: snapshot.pet_mode,
            pet_state: snapshot.pet_state,
            loopback_host: snapshot.loopback_host,
            loopback_port: snapshot.loopback_port,
            loopback_addr: snapshot.loopback_addr,
            loopback_error: snapshot.loopback_error,
            loopback_note: snapshot.loopback_note,
            provider: snapshot.provider,
            pets: snapshot.pets,
            active_pet_id: snapshot.active_pet_id,
            link_verified: snapshot.link_verified,
            ts: snapshot.ts,
        }
    }

    pub async fn set_phase(&self, ble: BlePhase, device: Option<String>) {
        let mut inner = self.inner.lock().await;
        inner.ble = ble;
        if device.is_some() {
            inner.device = device;
        }
        if ble == BlePhase::Disconnected {
            inner.device = None;
            inner.session_id = None;
            inner.secure_session = SecureSessionState::Disconnected;
            inner.simulated_mode = None;
            inner.simulated_state = None;
            inner.link_verified = false;
        } else if ble == BlePhase::Scanning {
            inner.secure_session = SecureSessionState::Disconnected;
            inner.session_id = None;
            inner.link_verified = false;
        }
    }

    pub async fn begin_secure_session(&self, id: &str, device: &str, enrolling: bool) {
        let mut inner = self.inner.lock().await;
        inner.ble = BlePhase::Scanning;
        inner.device = Some(device.to_string());
        inner.session_id = Some(id.to_string());
        inner.secure_session = SecureSessionState::Handshaking;
        if enrolling {
            inner.enrollment = EnrollmentPhase::PasskeyPending;
        }
    }

    pub async fn mark_secure_session_ready(&self, id: &str, device: &str) {
        let mut inner = self.inner.lock().await;
        inner.ble = BlePhase::Connected;
        inner.device = Some(device.to_string());
        inner.session_id = Some(id.to_string());
        inner.secure_session = SecureSessionState::Ready;
        if inner.enrollment != EnrollmentPhase::Idle {
            inner.enrollment = EnrollmentPhase::Success;
        }
    }

    pub async fn set_link_verified(&self, verified: bool) {
        self.inner.lock().await.link_verified = verified;
    }

    pub async fn is_link_verified(&self) -> bool {
        self.inner.lock().await.link_verified
    }

    pub async fn reset_secure_session(&self, expected_id: Option<&str>) {
        let mut inner = self.inner.lock().await;
        if expected_id.is_some() && inner.session_id.as_deref() != expected_id {
            return;
        }
        inner.ble = BlePhase::Disconnected;
        inner.device = None;
        inner.session_id = None;
        inner.secure_session = SecureSessionState::Disconnected;
        inner.simulated_mode = None;
        inner.simulated_state = None;
        inner.link_verified = false;
    }

    pub async fn set_enrollment(&self, phase: EnrollmentPhase) {
        self.inner.lock().await.enrollment = phase;
    }

    pub async fn set_pet_status(&self, mode: Option<String>, state: Option<String>) {
        let mut inner = self.inner.lock().await;
        if mode.is_some() {
            inner.simulated_mode = mode;
        }
        if state.is_some() {
            inner.simulated_state = state;
        }
    }

    pub async fn set_pet_status_for_session(
        &self,
        id: &str,
        mode: Option<String>,
        state: Option<String>,
    ) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.session_id.as_deref() != Some(id) {
            return false;
        }
        if mode.is_some() {
            inner.simulated_mode = mode;
        }
        if state.is_some() {
            inner.simulated_state = state;
        }
        true
    }

    pub async fn set_pairing(&self, active: bool) {
        let mut inner = self.inner.lock().await;
        inner.pairing_active = active;
        if !active {
            inner.pairing_deadline = None;
        }
    }

    pub async fn set_pairing_deadline(&self, deadline: Option<Instant>) {
        let mut inner = self.inner.lock().await;
        inner.pairing_deadline = deadline;
        inner.pairing_active = deadline.is_some();
    }

    pub async fn record_event(&self, event: &str) {
        self.inner.lock().await.last_event = Some(event.to_string());
    }

    pub async fn set_error(&self, error: Option<String>) {
        self.inner.lock().await.last_error = error;
    }

    pub async fn set_simulated_pet(&self, mode: &str, state: &str) {
        let mut inner = self.inner.lock().await;
        inner.simulated_mode = Some(mode.to_string());
        inner.simulated_state = Some(state.to_string());
    }

    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.ble == BlePhase::Connected && inner.secure_session == SecureSessionState::Ready
    }

    pub async fn pet_mode(&self) -> Option<String> {
        self.inner.lock().await.simulated_mode.clone()
    }

    pub async fn enqueue(&self, event: DeskPetEvent) -> usize {
        let mut queue = self.outgoing.lock().await;
        queue.push(event);
        queue.len()
    }

    pub async fn drain_queue(&self) -> Vec<DeskPetEvent> {
        self.outgoing.lock().await.drain()
    }

    pub fn subscribe_port(&self) -> watch::Receiver<u16> {
        self.port_tx.subscribe()
    }

    pub fn request_port(&self, port: u16) -> Result<u16, String> {
        let port = loopback::validate_port(port)?;
        let _ = self.port_tx.send_if_modified(|current| {
            if *current == port {
                false
            } else {
                *current = port;
                true
            }
        });
        Ok(port)
    }

    pub async fn is_listening_on(&self, port: u16) -> bool {
        let loopback = self.loopback.lock().await;
        loopback.error.is_none() && loopback.addr.is_some() && loopback.port == port
    }

    pub async fn set_loopback(
        &self,
        addr: Option<String>,
        port: u16,
        error: Option<String>,
        note: Option<String>,
    ) {
        let mut loopback = self.loopback.lock().await;
        loopback.addr = addr;
        loopback.port = if port == 0 { DEFAULT_PORT } else { port };
        loopback.error = error;
        loopback.note = note;
    }

    pub async fn set_provider(&self, provider: String) -> Result<String, String> {
        let provider = loopback::save_provider(&provider)?;
        *self.provider.lock().await = provider.clone();
        Ok(provider)
    }

    pub async fn roster_snapshot(&self) -> loopback::PetRoster {
        self.roster.lock().await.clone()
    }

    pub async fn remember_pet(&self, id: String, name: String) {
        let mut roster = self.roster.lock().await;
        if let Some(existing) = roster.pets.iter_mut().find(|pet| pet.id == id) {
            existing.name = name;
        } else {
            roster.pets.push(loopback::KnownPet {
                id: id.clone(),
                name,
            });
        }
        roster.active_pet_id = Some(id);
        persist_roster(&roster);
    }

    pub async fn set_active_pet(&self, id: String) -> Result<String, String> {
        let mut roster = self.roster.lock().await;
        let exists = roster.pets.iter().any(|pet| pet.id == id || pet.name == id);
        if !exists {
            return Err("That pet is not paired yet.".to_string());
        }
        let resolved = roster
            .pets
            .iter()
            .find(|pet| pet.id == id || pet.name == id)
            .map(|pet| pet.id.clone())
            .unwrap_or(id);
        roster.active_pet_id = Some(resolved.clone());
        persist_roster(&roster);
        Ok(resolved)
    }

    pub async fn forget_active_pet(&self) -> Option<loopback::KnownPet> {
        let mut roster = self.roster.lock().await;
        let active = roster.active_pet_id.clone()?;
        let removed = roster
            .pets
            .iter()
            .position(|pet| pet.id == active || pet.name == active)
            .map(|index| roster.pets.remove(index));
        roster.active_pet_id = roster.pets.first().map(|pet| pet.id.clone());
        persist_roster(&roster);
        removed
    }
}

fn persist_roster(roster: &loopback::PetRoster) {
    if cfg!(test) {
        return;
    }
    let _ = loopback::save_roster(roster);
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
