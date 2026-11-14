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
//! Protocol 5: tablet updates pushed by the PC. `hello` carries the tablet
//! app's version and whether it can install an update silently (kiosk mode);
//! `welcome.mode` says whether the session is a normal one or limited to
//! updating (`update_only`, when the two sides speak different protocol
//! versions); `update:offer` / `update:request` / `update:result`, the APK
//! itself in binary frames.
//!
//! **The frozen core.** `hello`, `welcome`, `error`, `ping`, `pong` and the
//! `update:*` messages, with the binary frames, never change in a breaking
//! way, whatever [`PROTOCOL_VERSION`] becomes: they may only gain optional
//! fields. A tablet from protocol [`FIRST_UPDATABLE_VERSION`] on can then
//! always be updated by a newer PC, and tell a user that the PC is the one
//! to update. Tablets from protocol 4 and before predate it: only USB
//! (`adb install`) updates them. Each version's `hello` and `welcome` are
//! kept as fixtures (`fixtures/core/`) and must keep decoding.
//!
//! Protocol 4: the tablet reports its kiosk mode (`tablet:status`) and the
//! PC can take it out of it (`tablet:command`); the PC sends the tablet's
//! settings (`tablet:settings`, screensaver) and the states shown on the
//! screensaver (`system:status`). Each is sent after `welcome`, then on change.
//!
//! Protocol 3: folders in the layout, button state (`active`, `unavailable`,
//! `badge`), `button:update` and `navigate`. The layout always carries the
//! current state of every button, so a tablet that connects is up to date;
//! changes then come as `button:update`.
//!
//! Authentication (protocol 2):
//! - The tablet sends `hello` with its `deviceId` and, once paired, its token.
//! - `welcome.auth` tells whether the session is authenticated. Connections on
//!   the USB port (loopback, via `adb reverse`) are always authenticated.
//! - Until authenticated, the tablet may only send `pair:request`; anything
//!   else gets an `unauthorized` error and the connection is closed.
//! - `pair:success` authenticates the current connection and carries the
//!   token to present in the next `hello`s.

use std::fmt;

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

mod fingerprint;
mod layout;

pub use fingerprint::{short_fingerprint, spki_sha256};
pub use layout::{ButtonIcon, ButtonView, Category, Folder, GridSize, Layout, Page, ProfileInfo};

/// Version of the wire protocol. Checked during the `hello` / `welcome` handshake.
pub const PROTOCOL_VERSION: u32 = 5;

/// The first protocol with the frozen core (see the crate documentation): a
/// tablet speaking it or a later one can be updated over Wi-Fi by any PC.
pub const FIRST_UPDATABLE_VERSION: u32 = 5;

/// Returns `true` if a peer speaking `version` can talk to us.
#[must_use]
pub fn is_compatible(version: u32) -> bool {
    version == PROTOCOL_VERSION
}

/// Returns `true` if a peer speaking `version` knows the frozen core: when
/// not [compatible](is_compatible), its session can still be limited to
/// updating.
#[must_use]
pub fn speaks_core(version: u32) -> bool {
    version >= FIRST_UPDATABLE_VERSION
}

/// Size of the APK pieces of an update (`update:request`): the last one is
/// shorter.
pub const UPDATE_CHUNK_SIZE: u32 = 64 * 1024;

/// Largest APK the PC offers.
pub const MAX_UPDATE_SIZE: u32 = 256 * 1024 * 1024;

/// The binary frame carrying the APK bytes from `offset` (an
/// `update:request`): the offset as 4 big-endian bytes, then the bytes.
#[must_use]
pub fn update_chunk_frame(offset: u32, bytes: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(4 + bytes.len());
    frame.extend_from_slice(&offset.to_be_bytes());
    frame.extend_from_slice(bytes);
    frame
}

/// Reads an APK piece frame: its offset and bytes (`None` if shorter than
/// the offset).
#[must_use]
pub fn parse_update_chunk(frame: &[u8]) -> Option<(u32, &[u8])> {
    let (offset, bytes) = frame.split_first_chunk::<4>()?;
    Some((u32::from_be_bytes(*offset), bytes))
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
    /// Desktop → tablet: new appearance of one button (state change), same
    /// id. Replaces it wherever it is (page or folder); unknown ids are ignored.
    #[serde(rename = "button:update")]
    ButtonUpdate(ButtonView),
    /// Desktop → tablet: show another page or folder (a navigation button
    /// was pressed on this tablet).
    #[serde(rename = "navigate")]
    Navigate(NavigatePayload),
    /// Desktop → tablet: the PIN was refused. The connection stays open
    /// (unauthenticated) so the user can try again.
    #[serde(rename = "pair:failure")]
    PairFailure(PairFailurePayload),
    /// Tablet → desktop: what the tablet is doing (kiosk mode), sent once
    /// authenticated and on every change.
    #[serde(rename = "tablet:status")]
    TabletStatus(TabletStatusPayload),
    /// Desktop → tablet: an order given from the PC (status screen).
    #[serde(rename = "tablet:command")]
    TabletCommand(TabletCommandPayload),
    /// Desktop → tablet: the tablet's settings, kept on the PC. Sent once
    /// authenticated and on every change.
    #[serde(rename = "tablet:settings")]
    TabletSettings(TabletSettingsPayload),
    /// Desktop → tablet: PC states shown on the screensaver. Sent once
    /// authenticated and on every change.
    #[serde(rename = "system:status")]
    SystemStatus(SystemStatusPayload),
    /// Desktop → tablet (frozen core): a newer version of the app, sent when
    /// the user asks for it on the PC. The tablet answers with
    /// `update:request`s, or an `update:result` refusing it.
    #[serde(rename = "update:offer")]
    UpdateOffer(UpdateOfferPayload),
    /// Tablet → desktop (frozen core): the APK piece starting at `offset`
    /// ([`UPDATE_CHUNK_SIZE`] bytes, fewer for the last), sent back in a
    /// binary frame ([`update_chunk_frame`]). One request at a time.
    #[serde(rename = "update:request")]
    UpdateRequest(UpdateRequestPayload),
    /// Tablet → desktop (frozen core): how the offered update ended on the
    /// tablet's side.
    #[serde(rename = "update:result")]
    UpdateResult(UpdateResultPayload),
}

#[typeshare]
// Debug by hand: the secret is redacted.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The tablet app's version (`versionName`, e.g. "0.2.0"). Protocol 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// The tablet app's `versionCode`, which updates must exceed. Protocol 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version_code: Option<u32>,
    /// The app can install an update without asking (kiosk mode: it is the
    /// device owner). Only such tablets are offered updates over Wi-Fi.
    /// Protocol 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silent_install: Option<bool>,
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
    /// What the session allows. Absent before protocol 5 (always `full`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SessionMode>,
}

/// What an authenticated session allows, announced in `welcome`. Frozen
/// core: never gains a value (an older tablet could not read it).
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// Same protocol: the grid, presses, settings, states.
    Full,
    /// Different protocols (`welcome.protocolVersion` tells which side is
    /// older): only the frozen core. The PC offers an update when the tablet
    /// is the older one; otherwise the PC is the one to update.
    UpdateOnly,
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
// Debug by hand: the secret is redacted.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequestPayload {
    /// The 6 digits shown on the desktop.
    pub pin: String,
}

#[typeshare]
// Debug by hand: the secret is redacted.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairSuccessPayload {
    /// Secret to present in the next `hello`s (base64url, 32 random bytes).
    pub token: String,
}

/// Printed in place of a secret (token, PIN, password) by the `Debug`
/// implementations, so `{:?}` in a log never reveals one.
pub const REDACTED: &str = "<redacted>";

impl fmt::Debug for HelloPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelloPayload")
            .field("protocol_version", &self.protocol_version)
            .field("device_name", &self.device_name)
            .field("device_id", &self.device_id)
            .field("token", &self.token.as_ref().map(|_| REDACTED))
            .field("app_version", &self.app_version)
            .field("app_version_code", &self.app_version_code)
            .field("silent_install", &self.silent_install)
            .finish()
    }
}

impl fmt::Debug for PairRequestPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairRequestPayload")
            .field("pin", &REDACTED)
            .finish()
    }
}

impl fmt::Debug for PairSuccessPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairSuccessPayload")
            .field("token", &REDACTED)
            .finish()
    }
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
pub struct NavigatePayload {
    pub to: NavigateTarget,
    /// Page or folder id, for `page` and `folder`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Where `navigate` goes. Relative moves are resolved by the tablet, which
/// knows the page it shows; unknown ids are ignored.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigateTarget {
    /// The page `id` (closes any open folder).
    Page,
    /// The folder `id`, on top of what is shown.
    Folder,
    /// The next page (none after the last one).
    Next,
    /// The previous page.
    Previous,
    /// Closes the open folder (nothing if none is open).
    Back,
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

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabletStatusPayload {
    /// Kiosk mode: the app is the tablet's device owner, locked in front.
    pub kiosk: bool,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabletCommandPayload {
    pub order: TabletOrder,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabletOrder {
    /// Leave kiosk mode for good: the app stops being the device owner
    /// (setting it again takes `adb`).
    ExitKiosk,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabletSettingsPayload {
    /// Black screen with a clock after `screensaverDelaySeconds` without a touch.
    pub screensaver: bool,
    pub screensaver_delay_seconds: u32,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusPayload {
    /// The PC's default microphone is muted.
    pub microphone_muted: bool,
    /// OBS is recording.
    pub recording: bool,
    /// OBS is streaming.
    pub streaming: bool,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOfferPayload {
    /// `versionName` of the offered APK.
    pub version: String,
    /// `versionCode` of the offered APK: the tablet refuses one that is not
    /// above its own.
    pub version_code: u32,
    /// APK size in bytes (at most [`MAX_UPDATE_SIZE`]).
    pub size: u32,
    /// SHA-256 of the APK, lowercase hex: checked by the tablet once
    /// received, before its own checks of the APK's signature.
    pub sha256: String,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequestPayload {
    /// Start of the piece, a multiple of [`UPDATE_CHUNK_SIZE`] below `size`.
    pub offset: u32,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResultPayload {
    pub outcome: UpdateOutcome,
}

/// How an offered update ended on the tablet. Values may be added: only a
/// tablet older than (or as old as) the PC that offered sends it.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateOutcome {
    /// Checked and handed to Android's installer: the app restarts in the
    /// new version (the connection drops).
    Installing,
    /// Not in kiosk mode: the tablet cannot install without asking. Use USB.
    NotDeviceOwner,
    /// Not newer than the installed version (never installed: no downgrade).
    NotNewer,
    /// Another app's package.
    WrongPackage,
    /// Not signed with the installed app's certificate (or, in a release
    /// build, the release certificate).
    BadCertificate,
    /// The bytes received do not match `sha256` or `size`.
    BadHash,
    /// The transfer stopped or the file could not be written.
    TransferFailed,
    /// Android's installer refused it.
    InstallFailed,
    /// Another update is already in progress on the tablet.
    Busy,
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
            Self::ButtonUpdate(_) => "button:update",
            Self::Navigate(_) => "navigate",
            Self::TabletStatus(_) => "tablet:status",
            Self::TabletCommand(_) => "tablet:command",
            Self::TabletSettings(_) => "tablet:settings",
            Self::SystemStatus(_) => "system:status",
            Self::UpdateOffer(_) => "update:offer",
            Self::UpdateRequest(_) => "update:request",
            Self::UpdateResult(_) => "update:result",
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
