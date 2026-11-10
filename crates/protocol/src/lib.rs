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
//!
//! Authentication (protocol 2):
//! - The tablet sends `hello` with its `deviceId` and, once paired, its token.
//! - `welcome.auth` tells whether the session is authenticated. Connections on
//!   the USB port (loopback, via `adb reverse`) are always authenticated.
//! - Until authenticated, the tablet may only send `pair:request`; anything
//!   else gets an `unauthorized` error and the connection is closed.
//! - `pair:success` authenticates the current connection and carries the
//!   token to present in the next `hello`s.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

mod fingerprint;
mod layout;

pub use fingerprint::{short_fingerprint, spki_sha256};
pub use layout::{ButtonIcon, ButtonView, Category, GridSize, Layout, Page, ProfileInfo};

/// Version of the wire protocol. Checked during the `hello` / `welcome` handshake.
pub const PROTOCOL_VERSION: u32 = 2;

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
    /// Tablet → desktop: pairing attempt with the PIN shown on the desktop.
    #[serde(rename = "pair:request")]
    PairRequest(PairRequestPayload),
    /// Desktop → tablet: the PIN was right. The connection is now
    /// authenticated; the tablet stores the token for its next `hello`s.
    #[serde(rename = "pair:success")]
    PairSuccess(PairSuccessPayload),
    /// Desktop → tablet: the PIN was refused. The connection stays open
    /// (unauthenticated) so the user can try again.
    #[serde(rename = "pair:failure")]
    PairFailure(PairFailurePayload),
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloPayload {
    pub protocol_version: u32,
    /// Human-readable name of the tablet (e.g. "Galaxy Tab A").
    pub device_name: String,
    /// Stable random identifier of the tablet (UUID), created on first launch.
    pub device_id: String,
    /// Token received in `pair:success`. Absent before pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomePayload {
    pub protocol_version: u32,
    /// Human-readable name of the PC (hostname), shown in the tablet's top bar.
    pub server_name: String,
    /// Stable random identifier of the PC (UUID). The tablet only presents its
    /// token to the PC it paired with.
    pub server_id: String,
    pub auth: AuthStatus,
}

/// Authentication state of a connection, announced in `welcome`.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    /// Valid token, or USB connection: everything is allowed.
    Authenticated,
    /// No token: only `pair:request` is allowed.
    PairingRequired,
    /// The token is unknown (revoked, or the PC was reset). The tablet forgets
    /// it; only `pair:request` is allowed.
    TokenRejected,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequestPayload {
    /// The 6 digits shown on the desktop.
    pub pin: String,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairSuccessPayload {
    /// Secret to present in the next `hello`s (base64url, 32 random bytes).
    pub token: String,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairFailurePayload {
    pub reason: PairFailureReason,
    /// Attempts left with the current PIN (0 when it can no longer be used).
    pub attempts_left: u32,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairFailureReason {
    /// Wrong PIN; retry while `attemptsLeft` > 0.
    InvalidPin,
    /// The PIN expired: a new one must be generated on the desktop.
    Expired,
    /// Too many wrong attempts: a new PIN must be generated on the desktop.
    Locked,
    /// No pairing in progress on the desktop.
    NoActivePin,
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
    /// The message requires an authenticated connection. Fatal.
    Unauthorized,
    /// This device's pairing was revoked from the desktop. Fatal; the tablet
    /// forgets its token.
    Revoked,
    /// A newer connection of the same device replaced this one (e.g. USB
    /// plugged in while on Wi-Fi). Fatal; the tablet keeps the newer one.
    Superseded,
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
            Self::PairRequest(_) => "pair:request",
            Self::PairSuccess(_) => "pair:success",
            Self::PairFailure(_) => "pair:failure",
        }
    }
}

/// Default port of the USB transport: plain WebSocket on the PC's loopback
/// interface, reached by the tablet through `adb reverse` at `127.0.0.1`.
pub const USB_PORT: u16 = 47321;
/// Default port of the Wi-Fi transport: WebSocket over TLS (`wss://`), the
/// tablet pins the PC's public key (see [`short_fingerprint`]).
pub const WIFI_PORT: u16 = 47322;
/// DNS-SD service type announced over mDNS while Wi-Fi is enabled.
pub const MDNS_SERVICE_TYPE: &str = "_padboard._tcp.local.";
/// TXT record keys of the mDNS announcement.
pub mod txt {
    /// Protocol version spoken by the PC (decimal).
    pub const PROTOCOL_VERSION: &str = "v";
    /// `serverId` of the PC, as in `welcome`.
    pub const SERVER_ID: &str = "id";
    /// Human-readable name of the PC, as in `welcome`.
    pub const SERVER_NAME: &str = "name";
}
