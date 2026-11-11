//! Cross-language contract: every file in `fixtures/` must decode into the
//! expected message and re-encode to the exact same JSON. The Android unit
//! tests decode the same files.

// Helpers below are not #[test] functions, so clippy.toml's allow-unwrap-in-tests
// does not cover them.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use padboard_protocol::{Message, short_fingerprint, spki_sha256};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Fixture file stem for a message. The match is exhaustive on purpose:
/// adding a variant to `Message` breaks compilation here until a fixture
/// name is chosen (and the fixture written).
fn fixture_stem(message: &Message) -> &'static str {
    match message {
        Message::Hello(_) => "hello",
        Message::Welcome(_) => "welcome",
        Message::Error(_) => "error",
        Message::LayoutSync(_) => "layout_sync",
        Message::ButtonPress(_) => "button_press",
        Message::ButtonRelease(_) => "button_release",
        Message::Ping(_) => "ping",
        Message::Pong(_) => "pong",
        Message::PairRequest(_) => "pair_request",
        Message::PairSuccess(_) => "pair_success",
        Message::PairFailure(_) => "pair_failure",
        Message::ButtonUpdate(_) => "button_update",
        Message::Navigate(_) => "navigate",
        Message::TabletStatus(_) => "tablet_status",
        Message::TabletCommand(_) => "tablet_command",
        Message::TabletSettings(_) => "tablet_settings",
        Message::SystemStatus(_) => "system_status",
    }
}

fn load_fixtures() -> Vec<(String, Value)> {
    let mut out: Vec<_> = fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .map(|p| {
            let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
            let value: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap())
                .unwrap_or_else(|e| panic!("{stem}.json is not valid JSON: {e}"));
            (stem, value)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn every_fixture_decodes_and_reencodes_identically() {
    for (stem, original) in load_fixtures() {
        let message: Message = serde_json::from_value(original.clone())
            .unwrap_or_else(|e| panic!("{stem}.json does not decode: {e}"));

        assert_eq!(
            fixture_stem(&message),
            stem,
            "{stem}.json has the wrong type"
        );
        assert_eq!(
            original["type"].as_str(),
            Some(message.type_tag()),
            "{stem}.json: type tag mismatch"
        );

        let reencoded = serde_json::to_value(&message).unwrap();
        assert_eq!(reencoded, original, "{stem}.json does not round-trip");
    }
}

#[test]
fn every_message_type_has_a_fixture() {
    let found: BTreeSet<String> = load_fixtures().into_iter().map(|(s, _)| s).collect();
    let expected: BTreeSet<String> = [
        "hello",
        "welcome",
        "error",
        "layout_sync",
        "button_press",
        "button_release",
        "ping",
        "pong",
        "pair_request",
        "pair_success",
        "pair_failure",
        "button_update",
        "navigate",
        "tablet_status",
        "tablet_command",
        "tablet_settings",
        "system_status",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(found, expected);
}

/// The handshake fixtures speak the current version: the Kotlin constant
/// (hand-written, `ProtocolJson.kt`) is checked against them on the tablet side.
#[test]
fn handshake_fixtures_speak_the_current_version() {
    let fixtures = load_fixtures();
    for stem in ["hello", "welcome"] {
        let (_, value) = fixtures.iter().find(|(s, _)| s == stem).unwrap();
        assert_eq!(
            value["payload"]["protocolVersion"],
            serde_json::json!(padboard_protocol::PROTOCOL_VERSION),
            "{stem}.json"
        );
    }
}

#[test]
fn layout_fixture_is_valid() {
    let (_, value) = load_fixtures()
        .into_iter()
        .find(|(s, _)| s == "layout_sync")
        .unwrap();
    let Message::LayoutSync(layout) = serde_json::from_value(value).unwrap() else {
        panic!("layout_sync.json is not a layout:sync message");
    };
    layout.validate().unwrap();
}

/// Test vector shared with the tablet (`fixtures/vectors/fingerprint.json`).
#[test]
fn fingerprint_vector() {
    let path = fixtures_dir().join("vectors").join("fingerprint.json");
    let vector: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let spki = hex(vector["spkiDerHex"].as_str().unwrap());
    let sha = hex(vector["sha256Hex"].as_str().unwrap());

    assert_eq!(spki_sha256(&spki).as_slice(), sha.as_slice());
    assert_eq!(
        short_fingerprint(&spki),
        vector["shortFingerprint"].as_str().unwrap()
    );
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
