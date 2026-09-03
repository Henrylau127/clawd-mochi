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

    pub fn remaining_seconds(&self) -> Option<u64> {
        self.remaining_seconds_from(Instant::now())
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn remaining_seconds_from(&self, now: Instant) -> Option<u64> {
        self.deadline.map(|deadline| {
            let remaining = deadline.saturating_duration_since(now);
            remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0)
        })
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

#[cfg(test)]
mod tests {
    use super::PairingWindow;

    #[test]
    fn remaining_seconds_is_empty_until_started() {
        assert_eq!(PairingWindow::new().remaining_seconds(), None);
    }

    #[test]
    fn starting_window_reports_its_configured_duration() {
        let mut window = PairingWindow::new();
        window.start();
        assert_eq!(
            window.remaining_seconds(),
            Some(PairingWindow::WINDOW.as_secs())
        );
    }
}
