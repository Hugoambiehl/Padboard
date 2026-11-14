//! Release signatures: minisign, in the format of the Tauri CLI
//! (`tauri signer sign --app-version X`), checked with the public key
//! embedded in the app (`updater-key.pub`).
//!
//! Two things are signed on the maintainer's PC (`scripts/sign-release.sh`):
//! the Windows installer (checked by `tauri-plugin-updater`) and
//! `SHA256SUMS` (checked here, before the tablet's APK is used). The
//! signature's trusted comment (`timestamp:…\tfile:…\tversion:…`) is covered
//! by the signature: it binds the file to its name and to one version, so an
//! older genuine `SHA256SUMS` cannot be served for a newer release.

use std::collections::HashMap;

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use sha2::{Digest as _, Sha256};

/// The update public key (`tauri signer generate`), base64 as the Tauri CLI
/// writes it. Empty until the maintainer's key exists: updates are then off.
const EMBEDDED_KEY: &str = include_str!("../../updater-key.pub");

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SigningError {
    #[error("no update key in this build")]
    NoKey,
    #[error("unreadable key or signature: {0}")]
    Malformed(String),
    #[error("bad signature")]
    BadSignature,
    #[error("signed for {signed:?}, expected {expected:?}")]
    WrongFile { signed: String, expected: String },
    #[error("signed for version {signed:?}, expected {expected}")]
    WrongVersion {
        signed: Option<String>,
        expected: String,
    },
    #[error("{0} is not listed in SHA256SUMS")]
    NotListed(String),
    #[error("{0} does not match its SHA-256")]
    HashMismatch(String),
}

/// A public key able to check release signatures.
#[derive(Clone)]
pub struct UpdateKey(PublicKey);

impl std::fmt::Debug for UpdateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UpdateKey")
    }
}

impl UpdateKey {
    /// The key this build trusts, `None` when the build has none.
    #[must_use]
    pub fn embedded() -> Option<Self> {
        if EMBEDDED_KEY.trim().is_empty() {
            return None;
        }
        match Self::from_tauri(EMBEDDED_KEY) {
            Ok(key) => Some(key),
            Err(e) => {
                tracing::error!("the embedded update key is unusable: {e}");
                None
            }
        }
    }

    /// The text of the embedded key, as `tauri-plugin-updater` takes it.
    #[must_use]
    pub fn embedded_text() -> &'static str {
        EMBEDDED_KEY.trim()
    }

    /// Reads a key in the Tauri CLI's format (`*.key.pub`: base64 of the
    /// minisign public key file).
    ///
    /// # Errors
    /// Not base64, or not a minisign public key.
    pub fn from_tauri(text: &str) -> Result<Self, SigningError> {
        let decoded = decode_base64_text(text)?;
        PublicKey::decode(&decoded)
            .map(Self)
            .map_err(|e| SigningError::Malformed(e.to_string()))
    }

    /// Checks `signature` (a Tauri `.sig`: base64 of a minisign signature
    /// file) over `data`, made for the file `name` of `version`.
    ///
    /// # Errors
    /// A bad signature, or one made for another file or version.
    pub fn verify(
        &self,
        data: &[u8],
        signature: &str,
        name: &str,
        version: &str,
    ) -> Result<(), SigningError> {
        let decoded = decode_base64_text(signature)?;
        let signature =
            Signature::decode(&decoded).map_err(|e| SigningError::Malformed(e.to_string()))?;
        self.0
            .verify(data, &signature, false)
            .map_err(|_| SigningError::BadSignature)?;
        // Only now is the trusted comment trustworthy: `verify` checked the
        // global signature that covers it.
        let comment = TrustedComment::parse(signature.trusted_comment());
        if comment.file != Some(name) {
            return Err(SigningError::WrongFile {
                signed: comment.file.unwrap_or_default().to_owned(),
                expected: name.to_owned(),
            });
        }
        if comment.version != Some(version) {
            return Err(SigningError::WrongVersion {
                signed: comment.version.map(str::to_owned),
                expected: version.to_owned(),
            });
        }
        Ok(())
    }
}

/// The fields of a Tauri CLI trusted comment: `key:value` pairs separated by
/// tabs.
#[derive(Debug, Default, PartialEq, Eq)]
struct TrustedComment<'a> {
    file: Option<&'a str>,
    version: Option<&'a str>,
}

impl<'a> TrustedComment<'a> {
    fn parse(text: &'a str) -> Self {
        let mut comment = Self::default();
        for field in text.split('\t') {
            if let Some(file) = field.strip_prefix("file:") {
                comment.file = Some(file);
            } else if let Some(version) = field.strip_prefix("version:") {
                comment.version = Some(version);
            }
        }
        comment
    }
}

fn decode_base64_text(text: &str) -> Result<String, SigningError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| SigningError::Malformed(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| SigningError::Malformed(e.to_string()))
}

/// A verified `SHA256SUMS`: file name → SHA-256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksums(HashMap<String, [u8; 32]>);

/// The name of the checksum file of a release, and of its signature.
pub const CHECKSUMS: &str = "SHA256SUMS";
pub const CHECKSUMS_SIGNATURE: &str = "SHA256SUMS.sig";

impl Checksums {
    /// Checks the signature of `SHA256SUMS` (made for this file name and
    /// `version`), then reads it.
    ///
    /// # Errors
    /// A bad signature, or lines that are not `sha256sum` output.
    pub fn verify(
        key: &UpdateKey,
        text: &[u8],
        signature: &str,
        version: &str,
    ) -> Result<Self, SigningError> {
        key.verify(text, signature, CHECKSUMS, version)?;
        let text = std::str::from_utf8(text).map_err(|e| SigningError::Malformed(e.to_string()))?;
        Self::parse(text)
    }

    /// `sha256sum` output: `<64 hex digits> <space><space or *><name>`.
    fn parse(text: &str) -> Result<Self, SigningError> {
        let mut sums = HashMap::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let malformed = || SigningError::Malformed(format!("SHA256SUMS line {line:?}"));
            let (hex, rest) = line.split_once(' ').ok_or_else(malformed)?;
            let name = rest
                .strip_prefix(' ')
                .or_else(|| rest.strip_prefix('*'))
                .ok_or_else(malformed)?;
            let digest = decode_hex(hex).ok_or_else(malformed)?;
            sums.insert(name.to_owned(), digest);
        }
        Ok(Self(sums))
    }

    /// Checks that `data` is the file `name` of the release.
    ///
    /// # Errors
    /// The file is not listed, or its content differs.
    pub fn check(&self, name: &str, data: &[u8]) -> Result<(), SigningError> {
        let expected = self
            .0
            .get(name)
            .ok_or_else(|| SigningError::NotListed(name.to_owned()))?;
        if Sha256::digest(data).as_slice() == expected {
            Ok(())
        } else {
            Err(SigningError::HashMismatch(name.to_owned()))
        }
    }

    /// The expected SHA-256 of `name`, if listed.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<[u8; 32]> {
        self.0.get(name).copied()
    }
}

fn decode_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Signed with a throwaway key (`tauri signer generate`), never used for
    // a release.
    const KEY: &str = include_str!("fixtures/test-key.pub");
    const SUMS: &[u8] = include_bytes!("fixtures/SHA256SUMS");
    const SIG: &str = include_str!("fixtures/SHA256SUMS.sig");
    const SIG_NO_VERSION: &str = include_str!("fixtures/SHA256SUMS-unversioned.sig");
    const SIG_OTHER_FILE: &str = include_str!("fixtures/SHA256SUMS-other-file.sig");
    const APK: &[u8] = b"not a real apk, version 1.2.3\n";

    fn key() -> UpdateKey {
        UpdateKey::from_tauri(KEY).unwrap()
    }

    #[test]
    fn genuine_checksums_are_accepted_and_check_files() {
        let sums = Checksums::verify(&key(), SUMS, SIG, "1.2.3").unwrap();
        sums.check("padboard-1.2.3.apk", APK).unwrap();
        assert_eq!(
            sums.check("padboard-1.2.3.apk", b"tampered"),
            Err(SigningError::HashMismatch("padboard-1.2.3.apk".into()))
        );
        assert_eq!(
            sums.check("padboard-9.9.9.apk", APK),
            Err(SigningError::NotListed("padboard-9.9.9.apk".into()))
        );
    }

    #[test]
    fn tampered_checksums_are_refused() {
        let mut tampered = SUMS.to_vec();
        tampered[0] = if tampered[0] == b'0' { b'1' } else { b'0' };
        assert_eq!(
            Checksums::verify(&key(), &tampered, SIG, "1.2.3"),
            Err(SigningError::BadSignature)
        );
    }

    #[test]
    fn another_key_is_refused() {
        // A key of the same format that did not sign the fixtures.
        let other = UpdateKey::from_tauri(include_str!("fixtures/other-key.pub")).unwrap();
        assert_eq!(
            Checksums::verify(&other, SUMS, SIG, "1.2.3"),
            Err(SigningError::BadSignature)
        );
    }

    #[test]
    fn checksums_of_another_version_are_refused() {
        // A genuine older SHA256SUMS served for a newer release.
        assert_eq!(
            Checksums::verify(&key(), SUMS, SIG, "1.3.0"),
            Err(SigningError::WrongVersion {
                signed: Some("1.2.3".into()),
                expected: "1.3.0".into()
            })
        );
        assert_eq!(
            Checksums::verify(&key(), SUMS, SIG_NO_VERSION, "1.2.3"),
            Err(SigningError::WrongVersion {
                signed: None,
                expected: "1.2.3".into()
            })
        );
    }

    #[test]
    fn a_signature_made_for_another_file_is_refused() {
        assert_eq!(
            Checksums::verify(&key(), SUMS, SIG_OTHER_FILE, "1.2.3"),
            Err(SigningError::WrongFile {
                signed: "OTHER".into(),
                expected: "SHA256SUMS".into()
            })
        );
    }

    #[test]
    fn garbage_is_malformed_not_a_panic() {
        assert!(matches!(
            UpdateKey::from_tauri("not base64 !"),
            Err(SigningError::Malformed(_))
        ));
        assert!(matches!(
            Checksums::verify(&key(), SUMS, "bm90IGEgc2lnbmF0dXJl", "1.2.3"),
            Err(SigningError::Malformed(_))
        ));
        assert!(matches!(
            Checksums::parse("abc  file"),
            Err(SigningError::Malformed(_))
        ));
    }

    #[test]
    fn checksum_lines_in_text_and_binary_mode() {
        let hex = "00".repeat(32);
        let sums = Checksums::parse(&format!("{hex}  a.txt\n{hex} *b.bin\n\n")).unwrap();
        assert_eq!(sums.get("a.txt"), Some([0; 32]));
        assert_eq!(sums.get("b.bin"), Some([0; 32]));
    }

    #[test]
    fn trusted_comment_fields() {
        assert_eq!(
            TrustedComment::parse("timestamp:1\tfile:SHA256SUMS\tversion:0.2.0"),
            TrustedComment {
                file: Some("SHA256SUMS"),
                version: Some("0.2.0")
            }
        );
        assert_eq!(TrustedComment::parse("anything"), TrustedComment::default());
    }
}
