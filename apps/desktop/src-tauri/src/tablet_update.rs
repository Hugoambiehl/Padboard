//! An update sent to a tablet over its connection (`update:*`, frozen core
//! of the protocol): only to tablets in kiosk mode (they install it
//! silently), whatever their protocol version. The APK was checked by the
//! PC before (signed `SHA256SUMS`, `updates::tablet`) and is checked again
//! by the tablet (hash, package, version, certificate).

use std::sync::Arc;

use padboard_protocol::{MAX_UPDATE_SIZE, UPDATE_CHUNK_SIZE, UpdateOfferPayload, UpdateOutcome};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

/// An APK ready to be sent: its version, its bytes and their SHA-256.
#[derive(Clone, PartialEq, Eq)]
pub struct UpdatePackage {
    pub version: String,
    pub version_code: u32,
    sha256: [u8; 32],
    bytes: Arc<[u8]>,
}

impl std::fmt::Debug for UpdatePackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdatePackage")
            .field("version", &self.version)
            .field("version_code", &self.version_code)
            .field("size", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl UpdatePackage {
    /// `None` when the APK is empty or larger than the protocol allows.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "built from the downloaded APK in the next step")
    )]
    #[must_use]
    pub fn new(version: String, version_code: u32, bytes: Vec<u8>) -> Option<Self> {
        let size = u32::try_from(bytes.len()).ok()?;
        if size == 0 || size > MAX_UPDATE_SIZE {
            return None;
        }
        Some(Self {
            version,
            version_code,
            sha256: Sha256::digest(&bytes).into(),
            bytes: bytes.into(),
        })
    }

    #[must_use]
    pub fn size(&self) -> u32 {
        // Checked in `new`.
        u32::try_from(self.bytes.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn offer(&self) -> UpdateOfferPayload {
        UpdateOfferPayload {
            version: self.version.clone(),
            version_code: self.version_code,
            size: self.size(),
            sha256: self.sha256.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }

    /// The piece at `offset`: a multiple of the chunk size inside the APK.
    #[must_use]
    pub fn chunk(&self, offset: u32) -> Option<&[u8]> {
        if !offset.is_multiple_of(UPDATE_CHUNK_SIZE) || offset >= self.size() {
            return None;
        }
        let start = usize::try_from(offset).ok()?;
        let end = start
            .saturating_add(UPDATE_CHUNK_SIZE as usize)
            .min(self.bytes.len());
        self.bytes.get(start..end)
    }
}

/// Where a tablet's update over its connection is, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TabletUpdate {
    /// Offered, bytes going out (`sent` of `total`).
    Sending {
        version: String,
        sent: u32,
        total: u32,
    },
    /// Checked by the tablet and handed to its installer: it restarts.
    Installing { version: String },
    /// Refused or failed on the tablet (`outcome`), see the protocol.
    Failed {
        version: String,
        outcome: UpdateOutcome,
    },
}

/// Why an update could not be offered to a connected tablet.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "offered from the UI in the next step")
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum OfferError {
    #[error("the tablet is no longer connected")]
    Gone,
    /// Not in kiosk mode: it cannot install without asking (USB instead).
    #[error("the tablet cannot install silently")]
    NotSilent,
    /// Its version is unknown (protocol 4) or already this one or newer.
    #[error("the tablet does not need this version")]
    NotNewer,
}

/// The update in progress on one connection.
#[derive(Debug)]
pub struct Transfer {
    pub package: Arc<UpdatePackage>,
    /// Bytes requested so far (the last request's end).
    sent: u32,
    /// Progress reported last, in 2 % steps (the UI does not need more).
    reported: u32,
}

impl Transfer {
    #[must_use]
    pub fn new(package: Arc<UpdatePackage>) -> Self {
        Self {
            package,
            sent: 0,
            reported: 0,
        }
    }

    /// Records a request answered; `Some` when the progress is worth
    /// reporting.
    pub fn served(&mut self, offset: u32, len: usize) -> Option<TabletUpdate> {
        let end = offset.saturating_add(u32::try_from(len).unwrap_or(u32::MAX));
        self.sent = self.sent.max(end);
        let total = self.package.size();
        let step = u32::try_from(u64::from(self.sent) * 50 / u64::from(total.max(1))).unwrap_or(50);
        if step == self.reported && self.sent < total {
            return None;
        }
        self.reported = step;
        Some(self.status())
    }

    #[must_use]
    pub fn status(&self) -> TabletUpdate {
        TabletUpdate::Sending {
            version: self.package.version.clone(),
            sent: self.sent,
            total: self.package.size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(len: usize) -> UpdatePackage {
        let bytes = (0..len).map(|i| (i % 251) as u8).collect();
        UpdatePackage::new("0.3.0".into(), 30_099, bytes).unwrap()
    }

    #[test]
    fn chunks_cover_the_apk_exactly() {
        let chunk = UPDATE_CHUNK_SIZE as usize;
        let p = package(2 * chunk + 10);
        let mut whole = Vec::new();
        let mut offset = 0;
        while let Some(bytes) = p.chunk(offset) {
            whole.extend_from_slice(bytes);
            offset += UPDATE_CHUNK_SIZE;
        }
        assert_eq!(whole.len(), 2 * chunk + 10);
        assert_eq!(&whole[..], &p.bytes[..]);
        assert_eq!(p.chunk(1), None, "not on a chunk boundary");
        assert_eq!(p.chunk(3 * UPDATE_CHUNK_SIZE), None, "past the end");
    }

    #[test]
    fn the_offer_describes_the_bytes() {
        let p = UpdatePackage::new("1.0.0".into(), 1_000_099, b"abc".to_vec()).unwrap();
        let offer = p.offer();
        assert_eq!(offer.size, 3);
        assert_eq!(
            offer.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(UpdatePackage::new("1.0.0".into(), 1, Vec::new()).is_none());
    }

    #[test]
    fn progress_is_reported_in_steps_and_at_the_end() {
        let chunk = UPDATE_CHUNK_SIZE as usize;
        let p = Arc::new(package(100 * chunk));
        let mut t = Transfer::new(Arc::clone(&p));
        let reports = (0..100u32)
            .filter_map(|i| t.served(i * UPDATE_CHUNK_SIZE, chunk))
            .count();
        assert_eq!(reports, 50);
        assert_eq!(
            t.status(),
            TabletUpdate::Sending {
                version: "0.3.0".into(),
                sent: p.size(),
                total: p.size()
            }
        );
    }
}
