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
    Send(DeskPetEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusSnapshot {
    pub ble: BlePhase,
    pub device: Option<String>,
    pub queue: usize,
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
            port_tx,
            _port_rx: port_rx,
            command_tx,
        }
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let inner = self.inner.lock().await;
        let queue = self.outgoing.lock().await.len();
        let loopback = self.loopback.lock().await;
        let port = loopback.port;
        StatusSnapshot {
            ble: inner.ble,
            device: inner.device.clone(),
            queue,
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
        self.port_tx.send(port).map_err(|err| err.to_string())?;
        Ok(port)
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
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
