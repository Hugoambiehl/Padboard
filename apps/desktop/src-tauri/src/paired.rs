//! Devices paired over Wi-Fi, persisted in `paired-devices.json` (owner-only).
//!
//! Only the SHA-256 of each token is stored: a leaked file does not let anyone
//! connect. Tokens are 32 random bytes, so a fast hash is enough (no dictionary
//! attack is possible, unlike passwords). Comparison is constant-time.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::storage::Document;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevices {
    pub devices: Vec<PairedDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub device_id: String,
    /// Name announced in `hello` when the device paired (updated on each connection).
    pub name: String,
    /// SHA-256 of the token, lowercase hex.
    pub token_sha256: String,
    /// Unix seconds.
    pub paired_at: u64,
    /// Unix seconds of the last authenticated connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<u64>,
}

impl Document for PairedDevices {
    const FILE: &'static str = "paired-devices.json";
    const VERSION: u32 = 1;
    const PRIVATE: bool = true;
}

impl PairedDevices {
    #[must_use]
    pub fn get(&self, device_id: &str) -> Option<&PairedDevice> {
        self.devices.iter().find(|d| d.device_id == device_id)
    }

    /// Whether `token` is the one issued to `device_id`.
    #[must_use]
    pub fn verify(&self, device_id: &str, token: &str) -> bool {
        let Some(device) = self.get(device_id) else {
            return false;
        };
        let presented = hex(&Sha256::digest(token.as_bytes()));
        // Same length (64 hex chars) by construction; ct_eq also handles a
        // tampered file with another length (returns false).
        presented
            .as_bytes()
            .ct_eq(device.token_sha256.as_bytes())
            .into()
    }

    /// Adds a device, replacing a previous pairing of the same device.
    pub fn insert(&mut self, device_id: &str, name: &str, token: &str, now: u64) {
        self.remove(device_id);
        self.devices.push(PairedDevice {
            device_id: device_id.to_owned(),
            name: name.to_owned(),
            token_sha256: hex(&Sha256::digest(token.as_bytes())),
            paired_at: now,
            last_seen: Some(now),
        });
    }

    /// Returns `true` if the device was paired.
    pub fn remove(&mut self, device_id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.device_id != device_id);
        self.devices.len() != before
    }

    /// Records an authenticated connection.
    pub fn touch(&mut self, device_id: &str, name: &str, now: u64) {
        if let Some(device) = self.devices.iter_mut().find(|d| d.device_id == device_id) {
            device.name = name.to_owned();
            device.last_seen = Some(now);
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use crate::storage::ConfigDir;

    use super::*;

    #[test]
    fn verify_accepts_only_the_issued_token_of_that_device() {
        let mut paired = PairedDevices::default();
        paired.insert("tab", "Galaxy Tab A", "secret-token", 100);
        assert!(paired.verify("tab", "secret-token"));
        assert!(!paired.verify("tab", "secret-tokeN"));
        assert!(!paired.verify("tab", ""));
        assert!(!paired.verify("other", "secret-token"));
    }

    #[test]
    fn stores_only_the_hash() {
        let mut paired = PairedDevices::default();
        paired.insert("tab", "Tab", "secret-token", 100);
        let json = serde_json::to_string(&paired).unwrap();
        assert!(!json.contains("secret-token"));
        assert_eq!(paired.devices[0].token_sha256.len(), 64);
    }

    #[test]
    fn repairing_replaces_and_remove_revokes() {
        let mut paired = PairedDevices::default();
        paired.insert("tab", "Tab", "old", 100);
        paired.insert("tab", "Tab", "new", 200);
        assert_eq!(paired.devices.len(), 1);
        assert!(!paired.verify("tab", "old"));
        assert!(paired.verify("tab", "new"));
        assert!(paired.remove("tab"));
        assert!(!paired.remove("tab"));
        assert!(!paired.verify("tab", "new"));
    }

    #[test]
    fn touch_updates_name_and_last_seen() {
        let mut paired = PairedDevices::default();
        paired.insert("tab", "Tab", "t", 100);
        paired.touch("tab", "Renamed", 300);
        paired.touch("unknown", "x", 300);
        assert_eq!(paired.devices[0].name, "Renamed");
        assert_eq!(paired.devices[0].last_seen, Some(300));
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(tmp.path()).unwrap();
        let mut paired = PairedDevices::default();
        paired.insert("tab", "Tab", "t", 1);
        config.save(&paired).unwrap();
        let mode = std::fs::metadata(config.path_of::<PairedDevices>())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(config.load::<PairedDevices>().unwrap(), Some(paired));
    }
}
