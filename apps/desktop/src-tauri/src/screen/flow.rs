//! Flow control of a stream: at most [`WINDOW`] frames sent and not yet
//! acknowledged by the tablet. The stream loop asks its source for a new
//! frame only while there is room, so a slow link or a slow decoder lowers
//! the frame rate instead of piling up latency (frames are dropped before
//! the encoder, never as encoded P-frames, which would break the picture).

use std::collections::VecDeque;

/// Frames in flight at most (`docs/design/second-screen.md`, section 4).
pub const WINDOW: usize = 2;

#[derive(Debug, Clone)]
pub struct Flow {
    window: usize,
    /// Capture times of the frames in flight, oldest first.
    in_flight: VecDeque<u64>,
}

impl Flow {
    #[must_use]
    pub fn new(window: usize) -> Self {
        Self {
            window: window.max(1),
            in_flight: VecDeque::new(),
        }
    }

    /// Whether another frame may be sent now.
    #[must_use]
    pub fn can_send(&self) -> bool {
        self.in_flight.len() < self.window
    }

    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// A frame captured at `capture_us` was sent.
    pub fn sent(&mut self, capture_us: u64) {
        self.in_flight.push_back(capture_us);
    }

    /// The tablet showed the frame captured at `capture_us`: it and every
    /// older frame are done (a decoder may skip a frame without showing
    /// it). An ack for a frame not in flight changes nothing.
    pub fn acked(&mut self, capture_us: u64) {
        if !self.in_flight.contains(&capture_us) {
            return;
        }
        while let Some(&oldest) = self.in_flight.front() {
            self.in_flight.pop_front();
            if oldest == capture_us {
                break;
            }
        }
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new(WINDOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_two_frames_are_in_flight() {
        let mut flow = Flow::default();
        assert!(flow.can_send());
        flow.sent(10);
        flow.sent(20);
        assert!(!flow.can_send());
        flow.acked(10);
        assert!(flow.can_send());
        assert_eq!(flow.in_flight(), 1);
    }

    #[test]
    fn an_ack_also_ends_older_frames() {
        let mut flow = Flow::new(3);
        flow.sent(10);
        flow.sent(20);
        flow.sent(30);
        flow.acked(20);
        assert_eq!(flow.in_flight(), 1);
        flow.acked(30);
        assert_eq!(flow.in_flight(), 0);
    }

    #[test]
    fn unknown_or_repeated_acks_change_nothing() {
        let mut flow = Flow::default();
        flow.sent(10);
        flow.sent(20);
        flow.acked(15);
        flow.acked(99);
        assert_eq!(flow.in_flight(), 2);
        flow.acked(10);
        flow.acked(10);
        assert_eq!(flow.in_flight(), 1);
    }
}
