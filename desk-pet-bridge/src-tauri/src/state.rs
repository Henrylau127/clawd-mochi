use serde::Serialize;
use tokio::sync::{mpsc, Mutex};

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
    pub ts: u64,
}

struct BridgeInner {
    ble: BlePhase,
    device: Option<String>,
    pairing_active: bool,
    last_event: Option<String>,
    last_error: Option<String>,
}

pub struct Shared {
    inner: Mutex<BridgeInner>,
    outgoing: Mutex<EventQueue>,
    pub command_tx: mpsc::Sender<BridgeCommand>,
}

impl Shared {
    pub fn new(command_tx: mpsc::Sender<BridgeCommand>) -> Self {
        Self {
            inner: Mutex::new(BridgeInner {
                ble: BlePhase::Disconnected,
                device: None,
                pairing_active: false,
                last_event: None,
                last_error: None,
            }),
            outgoing: Mutex::new(EventQueue::default()),
            command_tx,
        }
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let inner = self.inner.lock().await;
        let queue = self.outgoing.lock().await.len();
        StatusSnapshot {
            ble: inner.ble,
            device: inner.device.clone(),
            queue,
            pairing_active: inner.pairing_active,
            last_event: inner.last_event.clone(),
            last_error: inner.last_error.clone(),
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
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
