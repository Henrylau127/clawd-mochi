use serde::Serialize;
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
    pub device: Option<String>,
    pub queue: usize,
    pub queue_items: Vec<DeskPetEvent>,
    pub pairing_active: bool,
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
    device: Option<String>,
    pairing_active: bool,
    last_event: Option<String>,
    last_error: Option<String>,
    simulated_mode: Option<String>,
    simulated_state: Option<String>,
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
                device: None,
                pairing_active: false,
                last_event: None,
                last_error: None,
                simulated_mode: None,
                simulated_state: None,
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
        StatusSnapshot {
            ble: inner.ble,
            device: inner.device.clone(),
            queue,
            queue_items,
            pairing_active: inner.pairing_active,
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
            ts: unix_secs(),
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
        }
    }

    pub async fn set_pairing(&self, active: bool) {
        self.inner.lock().await.pairing_active = active;
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
        self.inner.lock().await.ble == BlePhase::Connected
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
        let exists = roster
            .pets
            .iter()
            .any(|pet| pet.id == id || pet.name == id);
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
