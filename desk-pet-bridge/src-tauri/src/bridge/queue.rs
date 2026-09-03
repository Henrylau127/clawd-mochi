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

    pub fn peek(&self) -> Vec<DeskPetEvent> {
        self.items.iter().cloned().collect()
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

    #[test]
    fn peek_preserves_order_without_draining() {
        let mut queue = EventQueue::default();
        queue.push(DeskPetEvent::new("sessionStart").with_state("coding"));
        queue.push(DeskPetEvent::new("stateUpdate").with_state("thinking"));
        let peeked = queue.peek();
        assert_eq!(peeked.len(), 2);
        assert_eq!(peeked[0].event, "sessionStart");
        assert_eq!(peeked[1].event, "stateUpdate");
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.drain().len(), 2);
        assert_eq!(queue.len(), 0);
    }
}
