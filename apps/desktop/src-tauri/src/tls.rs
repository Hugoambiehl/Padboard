//! TLS identity of the Wi-Fi port.
//!
//! Only the private key is stored (`tls-key.pem`, owner-only): the
//! self-signed certificate is rebuilt from it at every launch. Tablets pin the
//! SHA-256 of the public key (SPKI), not the certificate, so this never breaks
//! a pairing; a new key does (the tablet then has to pair again).
//!
//! Tablets do not check the certificate's names or dates (they only compare
//! the pinned key), so the certificate simply gets a long validity.

use std::sync::Arc;

use padboard_protocol::{short_fingerprint, spki_sha256};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PublicKeyData};
use rustls::ServerConfig;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::TlsAcceptor;

use crate::storage::{ConfigDir, StorageError};

const KEY_FILE: &str = "tls-key.pem";

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("cannot create the TLS key or certificate: {0}")]
    Rcgen(#[from] rcgen::Error),
    #[error("invalid TLS configuration: {0}")]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub struct TlsIdentity {
    pub acceptor: TlsAcceptor,
    /// Short code shown in the pairing dialog (`XXXX-XXXX-XXXX`).
    pub fingerprint: String,
    /// What the tablet pins (checked by the tests; the tablet computes it itself).
    #[cfg_attr(not(test), allow(dead_code))]
    pub spki_sha256: [u8; 32],
}

impl std::fmt::Debug for TlsIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsIdentity")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl TlsIdentity {
    /// Loads the key (creating it on first launch) and builds the certificate.
    ///
    /// # Errors
    /// If the key cannot be created or saved, or rustls refuses it.
    pub fn load_or_create(config: &ConfigDir) -> Result<Self, TlsError> {
        let key = load_or_create_key(config)?;
        Self::from_key(&key)
    }

    fn from_key(key: &KeyPair) -> Result<Self, TlsError> {
        let mut params = CertificateParams::new(vec!["padboard.local".to_owned()])?;
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, "Padboard");
        params.distinguished_name = name;
        params.not_before = rcgen::date_time_ymd(2025, 1, 1);
        params.not_after = rcgen::date_time_ymd(2099, 12, 31);
        let certificate = params.self_signed(key)?;

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
            )?;

        let spki = key.subject_public_key_info();
        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(config)),
            fingerprint: short_fingerprint(&spki),
            spki_sha256: spki_sha256(&spki),
        })
    }
}

fn load_or_create_key(config: &ConfigDir) -> Result<KeyPair, TlsError> {
    if let Some(bytes) = config.read_raw(KEY_FILE)? {
        match std::str::from_utf8(&bytes)
            .ok()
            .and_then(|pem| KeyPair::from_pem(pem).ok())
        {
            Some(key) => return Ok(key),
            None => {
                tracing::error!(
                    "{KEY_FILE} is unreadable: creating a new key, tablets must pair again"
                );
                config.move_raw_aside(KEY_FILE);
            }
        }
    }
    // ECDSA P-256, the rcgen default: small, fast, supported by every Android.
    let key = KeyPair::generate()?;
    config.write_secret(KEY_FILE, key.serialize_pem().as_bytes())?;
    tracing::info!("created the TLS key");
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> (tempfile::TempDir, ConfigDir) {
        let tmp = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(tmp.path()).unwrap();
        (tmp, config)
    }

    #[test]
    fn the_key_survives_restarts() {
        let (_tmp, config) = config();
        let first = TlsIdentity::load_or_create(&config).unwrap();
        let second = TlsIdentity::load_or_create(&config).unwrap();
        assert_eq!(first.spki_sha256, second.spki_sha256);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.fingerprint.len(), 14);
    }

    #[test]
    fn a_corrupt_key_is_replaced() {
        let (_tmp, config) = config();
        let first = TlsIdentity::load_or_create(&config).unwrap();
        config.write_secret(KEY_FILE, b"garbage").unwrap();
        let second = TlsIdentity::load_or_create(&config).unwrap();
        assert_ne!(first.spki_sha256, second.spki_sha256);
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, config) = config();
        TlsIdentity::load_or_create(&config).unwrap();
        let mode = std::fs::metadata(config.root().join(KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
