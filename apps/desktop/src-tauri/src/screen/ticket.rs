//! One-time tickets of the stream connection. `screen:offer` carries one;
//! the tablet presents it in the stream's handshake. A ticket is 32 random
//! bytes, kept only as its SHA-256 (looked up by hash, so its comparison
//! leaks nothing), valid [`VALIDITY`], used once.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::auth::{RandomError, random_token};

/// How long a ticket may wait for its stream connection.
pub const VALIDITY: Duration = Duration::from_secs(10);

type Key = [u8; 32];

fn key(ticket: &str) -> Key {
    Sha256::digest(ticket.as_bytes()).into()
}

/// What a ticket stands for, until used or expired.
#[derive(Debug)]
pub struct Tickets<T> {
    pending: HashMap<Key, (Instant, T)>,
}

impl<T> Default for Tickets<T> {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }
}

impl<T> Tickets<T> {
    /// A new ticket for `value`, valid from `now`.
    ///
    /// # Errors
    /// The OS random generator failed.
    pub fn issue(&mut self, value: T, now: Instant) -> Result<String, RandomError> {
        self.forget_expired(now);
        let ticket = random_token()?;
        self.pending.insert(key(&ticket), (now + VALIDITY, value));
        Ok(ticket)
    }

    /// The value of a valid ticket, which can then never be used again.
    pub fn redeem(&mut self, ticket: &str, now: Instant) -> Option<T> {
        let (expires, value) = self.pending.remove(&key(ticket))?;
        (now < expires).then_some(value)
    }

    /// Drops what `ticket` stood for, if still pending (an expired offer).
    pub fn take(&mut self, ticket: &str) -> Option<T> {
        self.pending.remove(&key(ticket)).map(|(_, value)| value)
    }

    /// Drops every pending ticket whose value matches.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.pending.retain(|_, (_, value)| keep(value));
    }

    fn forget_expired(&mut self, now: Instant) {
        self.pending.retain(|_, (expires, _)| now < *expires);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticket_is_used_once() {
        let mut tickets = Tickets::default();
        let now = Instant::now();
        let t = tickets.issue(7, now).unwrap();
        assert_eq!(t.len(), 43);
        assert_eq!(tickets.redeem(&t, now), Some(7));
        assert_eq!(tickets.redeem(&t, now), None);
    }

    #[test]
    fn an_expired_or_unknown_ticket_is_refused() {
        let mut tickets = Tickets::default();
        let now = Instant::now();
        let t = tickets.issue(1, now).unwrap();
        assert_eq!(tickets.redeem("not-a-ticket", now), None);
        assert_eq!(tickets.redeem(&t, now + VALIDITY), None);
    }

    #[test]
    fn expired_tickets_are_forgotten() {
        let mut tickets = Tickets::default();
        let now = Instant::now();
        tickets.issue(1, now).unwrap();
        tickets.issue(2, now + VALIDITY).unwrap();
        assert_eq!(tickets.len(), 1);
        tickets.retain(|&v| v != 2);
        assert_eq!(tickets.len(), 0);
    }

    #[test]
    fn tickets_differ() {
        let mut tickets = Tickets::default();
        let now = Instant::now();
        let a = tickets.issue((), now).unwrap();
        let b = tickets.issue((), now).unwrap();
        assert_ne!(a, b);
    }
}
