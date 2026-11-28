//! Who may start a second screen without asking (SECURITY.md): the PC's
//! user confirms the first time a device starts one, and the answer is
//! remembered only when that connection was verified (a paired Wi-Fi
//! session, or USB that passed the adb peer check), in
//! `screen-devices.json` (owner-only). Removed from the connections view,
//! and when the device's pairing is revoked.
//!
//! One question at a time (a source starts at most once at a time, see
//! [`super::Screens::starting`]); a new one replaces an older one, which
//! counts as refused.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::storage::{ConfigDir, Document};

/// How long the PC's user has to answer before it counts as refused.
pub const ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenDevices {
    pub devices: Vec<ScreenDevice>,
}

/// A device allowed to start a second screen without asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenDevice {
    pub device_id: String,
    /// Name announced in `hello` when it was allowed.
    pub name: String,
    /// Unix milliseconds.
    pub allowed_at_ms: u64,
}

impl Document for ScreenDevices {
    const FILE: &'static str = "screen-devices.json";
    const VERSION: u32 = 1;
    const PRIVATE: bool = true;
}

/// The allowed devices and the question waiting for an answer.
#[derive(Debug, Default)]
pub struct Consents {
    /// `None` in unit tests: kept in memory only.
    config: Option<ConfigDir>,
    allowed: Mutex<ScreenDevices>,
    pending: Mutex<Option<(u64, oneshot::Sender<bool>)>>,
    next_id: AtomicU64,
}

impl Consents {
    /// The devices remembered in `config`.
    #[must_use]
    pub fn load(config: ConfigDir) -> Self {
        let allowed = config.load_or_init(ScreenDevices::default);
        Self {
            config: Some(config),
            allowed: Mutex::new(allowed),
            ..Self::default()
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ScreenDevices> {
        self.allowed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn is_allowed(&self, device_id: &str) -> bool {
        self.lock().devices.iter().any(|d| d.device_id == device_id)
    }

    #[must_use]
    pub fn devices(&self) -> Vec<ScreenDevice> {
        self.lock().devices.clone()
    }

    /// Remembers `device_id` (its name refreshed if already there).
    pub fn remember(&self, device_id: &str, name: &str, now_ms: u64) {
        let mut allowed = self.lock();
        allowed.devices.retain(|d| d.device_id != device_id);
        allowed.devices.push(ScreenDevice {
            device_id: device_id.to_owned(),
            name: name.to_owned(),
            allowed_at_ms: now_ms,
        });
        self.save(&allowed);
    }

    /// Forgets `device_id`; false if it was not allowed.
    pub fn forget(&self, device_id: &str) -> bool {
        let mut allowed = self.lock();
        let before = allowed.devices.len();
        allowed.devices.retain(|d| d.device_id != device_id);
        let removed = allowed.devices.len() != before;
        if removed {
            self.save(&allowed);
        }
        removed
    }

    fn save(&self, allowed: &ScreenDevices) {
        if let Some(config) = &self.config
            && let Err(e) = config.save(allowed)
        {
            tracing::warn!(error = %e, "second screen: cannot save the allowed devices");
        }
    }

    /// A new question: its id (for the UI's answer) and where the answer
    /// comes. A question still waiting is dropped (refused).
    pub fn ask(&self) -> (u64, oneshot::Receiver<bool>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some((id, tx));
        (id, rx)
    }

    /// The PC's user answered question `id`; false if it is no longer
    /// waiting (answered, timed out, replaced).
    pub fn answer(&self, id: u64, allow: bool) -> bool {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        match pending.take() {
            Some((waiting, tx)) if waiting == id => tx.send(allow).is_ok(),
            other => {
                *pending = other;
                false
            }
        }
    }

    /// Question `id` is over (answered or timed out).
    pub fn close(&self, id: u64) {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.as_ref().is_some_and(|(waiting, _)| *waiting == id) {
            *pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_are_remembered_forgotten_and_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(dir.path()).unwrap();
        let consents = Consents::load(config.clone());
        assert!(!consents.is_allowed("tab"));
        consents.remember("tab", "Galaxy Tab A", 1);
        consents.remember("tab", "Galaxy Tab A (renamed)", 2);
        assert_eq!(consents.devices().len(), 1);
        assert_eq!(consents.devices()[0].name, "Galaxy Tab A (renamed)");
        // Read again from the file.
        assert!(Consents::load(config.clone()).is_allowed("tab"));
        assert!(consents.forget("tab"));
        assert!(!consents.forget("tab"));
        assert!(!Consents::load(config).is_allowed("tab"));
    }

    #[tokio::test]
    async fn an_answer_reaches_its_question_only() {
        let consents = Consents::default();
        let (first, first_rx) = consents.ask();
        let (second, second_rx) = consents.ask();
        // The first one was replaced: refused.
        assert!(first_rx.await.is_err());
        assert!(!consents.answer(first, true));
        assert!(consents.answer(second, true));
        assert_eq!(second_rx.await, Ok(true));
        // Answered: nothing waits any more.
        assert!(!consents.answer(second, false));
        let (third, _rx) = consents.ask();
        consents.close(third);
        assert!(!consents.answer(third, true));
    }
}
