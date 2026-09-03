use std::time::{Duration, Instant};

/// Tracks the local 120-second pairing window requested by the tray or API.
/// Actual LE Secure Connections pairing is performed by CoreBluetooth.
pub struct PairingWindow {
    deadline: Option<Instant>,
}

impl PairingWindow {
    pub const WINDOW: Duration = Duration::from_secs(120);

    pub fn new() -> Self {
        Self { deadline: None }
    }

    pub fn start(&mut self) {
        self.deadline = Some(Instant::now() + Self::WINDOW);
    }

    pub fn stop(&mut self) {
        self.deadline = None;
    }

    pub fn is_active(&self) -> bool {
        self.deadline
            .map(|deadline| Instant::now() < deadline)
            .unwrap_or(false)
    }

    pub fn expire_if_needed(&mut self) -> bool {
        if self.deadline.is_some() && !self.is_active() {
            self.deadline = None;
            true
        } else {
            false
        }
    }
}

impl Default for PairingWindow {
    fn default() -> Self {
        Self::new()
    }
}
