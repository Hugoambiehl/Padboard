//! Client side of the Wi-Fi TLS pinning, as the tablet does it: accept the
//! server certificate only if the SHA-256 of its public key (SPKI) is the
//! pinned one; names and dates are ignored.
//!
//! Not used by the app itself: it backs the end-to-end tests and the
//! `fake-tablet` example (manual tests without a tablet).

use std::sync::Arc;

use padboard_protocol::{short_fingerprint, spki_sha256};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, SignatureScheme};

/// What the client expects from the server's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    /// First contact (pairing): accept any key, the user compares the short
    /// fingerprint shown on both screens.
    TrustOnFirstUse,
    /// Paired: only this SPKI SHA-256 is accepted.
    Spki([u8; 32]),
}

/// rustls client configuration enforcing `pin`.
#[must_use]
pub fn client_config(pin: Pin) -> ClientConfig {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(PinVerifier {
        pin,
        provider: Arc::clone(&provider),
    });
    #[allow(clippy::expect_used)] // the ring provider supports the default versions
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth()
}

/// SHA-256 and short fingerprint of the key in a DER certificate.
#[must_use]
pub fn certificate_key(certificate: &[u8]) -> Option<([u8; 32], String)> {
    let spki = spki_of_certificate(certificate)?;
    Some((spki_sha256(spki), short_fingerprint(spki)))
}

#[derive(Debug)]
struct PinVerifier {
    pin: Pin,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let Some((hash, _)) = certificate_key(end_entity) else {
            return Err(Error::General("cannot read the certificate key".into()));
        };
        match self.pin {
            Pin::TrustOnFirstUse => Ok(ServerCertVerified::assertion()),
            Pin::Spki(pinned) if pinned == hash => Ok(ServerCertVerified::assertion()),
            Pin::Spki(_) => Err(Error::General("server key does not match the pin".into())),
        }
    }

    // The handshake signature must still be checked: it proves the server
    // holds the private key of the (pinned) certificate.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// The DER `SubjectPublicKeyInfo` of an X.509 certificate.
///
/// `Certificate ::= SEQUENCE { tbsCertificate, ... }` and
/// `TBSCertificate ::= SEQUENCE { [0] version OPTIONAL, serialNumber,
/// signature, issuer, validity, subject, subjectPublicKeyInfo, ... }`.
fn spki_of_certificate(der: &[u8]) -> Option<&[u8]> {
    const SEQUENCE: u8 = 0x30;
    const VERSION: u8 = 0xa0;
    let (tag, certificate, _) = read_tlv(der)?;
    if tag != SEQUENCE {
        return None;
    }
    let (tag, mut tbs, _) = read_tlv(certificate)?;
    if tag != SEQUENCE {
        return None;
    }
    if tbs.first() == Some(&VERSION) {
        tbs = read_tlv(tbs)?.2;
    }
    // serialNumber, signature, issuer, validity, subject.
    for _ in 0..5 {
        tbs = read_tlv(tbs)?.2;
    }
    let (tag, content, rest) = read_tlv(tbs)?;
    if tag != SEQUENCE {
        return None;
    }
    let whole_len = tbs.len() - rest.len();
    debug_assert!(whole_len > content.len());
    tbs.get(..whole_len)
}

/// Reads one DER element: `(tag, content, rest)`.
fn read_tlv(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, input) = input.split_first()?;
    let (&first, mut input) = input.split_first()?;
    let len = if first < 0x80 {
        usize::from(first)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > std::mem::size_of::<usize>() {
            return None;
        }
        let (bytes, rest) = input.split_at_checked(count)?;
        input = rest;
        bytes.iter().try_fold(0usize, |acc, &b| {
            acc.checked_mul(256)?.checked_add(usize::from(b))
        })?
    };
    let (content, rest) = input.split_at_checked(len)?;
    Some((tag, content, rest))
}

#[cfg(test)]
mod tests {
    use rcgen::{CertificateParams, KeyPair, PublicKeyData};

    use super::*;

    #[test]
    fn extracts_the_key_of_a_certificate() {
        let key = KeyPair::generate().unwrap();
        let certificate = CertificateParams::new(vec!["padboard.local".into()])
            .unwrap()
            .self_signed(&key)
            .unwrap();
        let spki = key.subject_public_key_info();
        assert_eq!(
            spki_of_certificate(certificate.der()),
            Some(spki.as_slice())
        );
        let (hash, code) = certificate_key(certificate.der()).unwrap();
        assert_eq!(hash, spki_sha256(&spki));
        assert_eq!(code, short_fingerprint(&spki));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            &b""[..],
            &[0x30],
            &[0x30, 0x05, 0x01],
            &[0x02, 0x00],
            &[0x30, 0x84, 0xff],
        ] {
            assert_eq!(spki_of_certificate(bad), None, "{bad:?}");
        }
    }
}
