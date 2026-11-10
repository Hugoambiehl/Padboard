//! Short, human-comparable fingerprint of the desktop's TLS public key.
//!
//! During Wi-Fi pairing, the desktop shows this code next to the PIN and the
//! tablet shows the code of the key it actually received over TLS. If both
//! screens show the same code, nobody sits between them (like Bluetooth
//! numeric comparison). The tablet then pins the full SHA-256 of the key.
//!
//! The code is computed from the DER-encoded `SubjectPublicKeyInfo` (SPKI),
//! not from the certificate: the certificate can be regenerated with the same
//! key without breaking pairings.
//!
//! Algorithm (implemented identically on the tablet, test vector in
//! `fixtures/vectors/fingerprint.json`):
//! 1. `h = SHA-256(spki_der)`;
//! 2. take the first 60 bits of `h` (big-endian);
//! 3. encode them as 12 characters of Crockford base32
//!    (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`, no I, L, O or U);
//! 4. group them by 4 with `-`: `XXXX-XXXX-XXXX`.
//!
//! Why 60 bits: the desktop key is long-lived and public, so an attacker can
//! search offline for a key with the same code for as long as they want.
//! 60 bits keeps that search far out of reach; a 6-digit code would not.

use sha2::{Digest, Sha256};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CHARS: usize = 12;
const GROUP: usize = 4;

/// SHA-256 of a DER-encoded `SubjectPublicKeyInfo`: the value the tablet pins.
#[must_use]
pub fn spki_sha256(spki_der: &[u8]) -> [u8; 32] {
    Sha256::digest(spki_der).into()
}

/// The 12-character code shown on both screens during pairing, e.g. `7K2Q-M9XD-4TRA`.
#[must_use]
pub fn short_fingerprint(spki_der: &[u8]) -> String {
    let hash = spki_sha256(spki_der);
    let mut first = [0u8; 8];
    first.copy_from_slice(&hash[..8]);
    let bits = u64::from_be_bytes(first);

    let mut code = String::with_capacity(CHARS + CHARS / GROUP - 1);
    for i in 0..CHARS {
        if i > 0 && i % GROUP == 0 {
            code.push('-');
        }
        let index = (bits >> (64 - 5 * (i + 1))) & 0x1f;
        // `index` < 32 by construction.
        code.push(char::from(ALPHABET[index as usize]));
    }
    code
}
