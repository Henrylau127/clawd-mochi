use std::collections::VecDeque;

use super::protocol::DeskPetEvent;

const MAX_QUEUE: usize = 64;

#[derive(Debug, Default)]
pub struct EventQueue {
    items: VecDeque<DeskPetEvent>,
}

impl EventQueue {
    pub fn push(&mut self, event: DeskPetEvent) {
        if self.items.len() >= MAX_QUEUE {
            self.items.pop_front();
        }
        self.items.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<DeskPetEvent> {
        self.items.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_oldest_when_full() {
        let mut queue = EventQueue::default();
        for _ in 0..70 {
            queue.push(DeskPetEvent::new("sessionStart").with_state("coding"));
        }
        assert_eq!(queue.len(), 64);
    }
}
