//! Padboard wire protocol: the single source of truth for messages exchanged
//! between the desktop app and the tablet.
//!
//! Every frame is a JSON text message shaped as `{ "type": "...", "payload": {...} }`.
//! TypeScript and Kotlin types are generated from this crate with
//! `scripts/gen-types.sh` — never edit the generated files by hand.
//!
//! Compatibility rules:
//! - Adding an optional field (`Option<T>`) is backward compatible: both
//!   clients ignore unknown keys.
//! - Anything else (renaming, removing, changing a type, adding a required
//!   field) is a breaking change and requires bumping [`PROTOCOL_VERSION`].
//!
//! Conventions:
//! - A payload struct must never have the same name as its `Message` variant:
//!   in the generated Kotlin, `data class Hello(val payload: Hello)` inside
//!   `sealed class Message` would refer to itself. Use a `…Payload` suffix.
//! - JSON fields are `camelCase`, enum values `snake_case`.
//! - Every message type has a fixture in `fixtures/` (decoded by Rust and Kotlin tests).

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

mod layout;

pub use layout::{ButtonView, Category, GridSize, Layout, Page, ProfileInfo};

/// Version of the wire protocol. Checked during the `hello` / `welcome` handshake.
pub const PROTOCOL_VERSION: u32 = 1;

/// Returns `true` if a peer speaking `version` can talk to us.
#[must_use]
pub fn is_compatible(version: u32) -> bool {
    version == PROTOCOL_VERSION
}

/// Envelope of every message exchanged over the WebSocket.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Message {
    /// Tablet → desktop, first message after the socket opens.
    #[serde(rename = "hello")]
    Hello(HelloPayload),
    /// Desktop → tablet, answer to a compatible `hello`.
    #[serde(rename = "welcome")]
    Welcome(WelcomePayload),
    /// Either direction. After a fatal error the sender closes the socket.
    #[serde(rename = "error")]
    Error(ErrorPayload),
    /// Desktop → tablet: full layout to display (replaces the previous one).
    #[serde(rename = "layout:sync")]
    LayoutSync(Layout),
    /// Tablet → desktop: a button was pressed.
    #[serde(rename = "button:press")]
    ButtonPress(ButtonEvent),
    /// Tablet → desktop: a button was released (used for long presses).
    #[serde(rename = "button:release")]
    ButtonRelease(ButtonEvent),
    /// Either direction: heartbeat request.
    #[serde(rename = "ping")]
    Ping(Heartbeat),
    /// Either direction: heartbeat answer, echoing the `ping` sequence number.
    #[serde(rename = "pong")]
    Pong(Heartbeat),
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloPayload {
    pub protocol_version: u32,
    /// Human-readable name of the tablet (e.g. "Galaxy Tab A").
    pub device_name: String,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomePayload {
    pub protocol_version: u32,
    /// Human-readable name of the PC (hostname), shown in the tablet's top bar.
    pub server_name: String,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: ErrorCode,
    /// Developer-oriented details (English). Clients display a localized text
    /// based on `code`, never this message.
    pub message: String,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The peer's protocol version is not supported. Fatal.
    UnsupportedVersion,
    /// The message could not be parsed or was not expected at this point.
    BadRequest,
    /// The referenced button does not exist in the current layout.
    UnknownButton,
    /// The action bound to the button failed on the PC.
    ActionFailed,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonEvent {
    pub button_id: String,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    /// Sequence number chosen by the `ping` sender, echoed in the `pong`.
    pub seq: u32,
}

impl Message {
    /// Serializes the message to its JSON wire form.
    ///
    /// # Errors
    /// Only fails if serialization itself fails, which cannot happen with the
    /// types of this crate.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Parses a message from its JSON wire form.
    ///
    /// # Errors
    /// Returns an error if the JSON is malformed or does not match any message.
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// The wire `type` tag of this message (e.g. `"button:press"`).
    #[must_use]
    pub fn type_tag(&self) -> &'static str {
        match self {
            Self::Hello(_) => "hello",
            Self::Welcome(_) => "welcome",
            Self::Error(_) => "error",
            Self::LayoutSync(_) => "layout:sync",
            Self::ButtonPress(_) => "button:press",
            Self::ButtonRelease(_) => "button:release",
            Self::Ping(_) => "ping",
            Self::Pong(_) => "pong",
        }
    }
}
