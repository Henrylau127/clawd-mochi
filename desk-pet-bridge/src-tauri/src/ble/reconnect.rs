use std::time::Duration;

pub struct Backoff {
    delay: Duration,
}

impl Backoff {
    pub fn new() -> Self {
        Self {
            delay: Duration::from_secs(1),
        }
    }

    pub fn next(&mut self) -> Duration {
        let current = self.delay;
        self.delay = (self.delay * 2).min(Duration::from_secs(30));
        current
    }

    pub fn reset(&mut self) {
        self.delay = Duration::from_secs(1);
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_at_thirty_seconds() {
        let mut backoff = Backoff::new();
        let mut last = Duration::from_secs(0);
        for _ in 0..8 {
            last = backoff.next();
        }
        assert_eq!(last, Duration::from_secs(30));
        backoff.reset();
        assert_eq!(backoff.next(), Duration::from_secs(1));
    }
}
