//! Second screen (#20, `docs/design/second-screen.md`): the tablet shows a
//! picture of the PC, streamed on a second WebSocket.
//!
//! - The tablet announces what it can decode in `hello` ([`ScreenCapability`],
//!   optional: protocol 5 stays, a tablet without it never gets a stream).
//! - The PC offers a stream on the session (`screen:offer`) with a one-time
//!   ticket. The tablet opens a second WebSocket on the same port and
//!   transport, path [`STREAM_PATH`], the ticket in the
//!   [`STREAM_TICKET_HEADER`] header of the handshake; it never sends `hello`
//!   there.
//! - On that stream connection only binary messages travel, framed by
//!   [`StreamFrame`]: video access units from the PC, acknowledgements and
//!   key frame requests from the tablet. Unknown kinds are ignored, so kinds
//!   can be added (touch input comes later).
//! - Either side ends it on the session: `screen:stop` (tablet),
//!   `screen:stopped` (PC), or by closing the stream connection.

use std::fmt;

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::REDACTED;

/// Path of the stream connection's WebSocket handshake.
pub const STREAM_PATH: &str = "/stream";

/// Handshake header carrying the ticket of `screen:offer` (lowercase:
/// HTTP header names are compared case-insensitively).
pub const STREAM_TICKET_HEADER: &str = "x-padboard-ticket";

/// What the tablet can show, in `hello` (optional).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCapability {
    /// The tablet's screen in pixels, landscape (width ≥ height).
    pub display_width: u32,
    pub display_height: u32,
    /// Hardware decoders, best first. The PC picks the first codec it can
    /// encode, at a size within that decoder's limits.
    pub decoders: Vec<ScreenDecoder>,
    /// The tablet sends touches as mouse input on the stream connection.
    pub touch: bool,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenDecoder {
    pub codec: VideoCodec,
    /// Largest picture the decoder takes (SM-T580's H.264: 1920×1088).
    pub max_width: u32,
    pub max_height: u32,
}

/// Video formats of the stream. Never gains a value without a capability
/// to announce it (an older tablet could not read it).
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    /// H.264, Annex B byte stream, parameter sets before each key frame.
    H264,
    /// VP8, one frame per access unit.
    Vp8,
}

/// PC → tablet: a stream is ready. Open the stream connection with
/// `ticket` within 10 seconds.
#[typeshare]
// Debug by hand: the ticket is a secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOfferPayload {
    /// One-time secret (base64url, 32 random bytes), bound to this session.
    pub ticket: String,
    pub codec: VideoCodec,
    /// Size of the pictures, within the chosen decoder's limits.
    pub width: u32,
    pub height: u32,
    /// Frames per second at most (fewer when the screen does not change).
    pub fps: u32,
}

impl fmt::Debug for ScreenOfferPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScreenOfferPayload")
            .field("ticket", &REDACTED)
            .field("codec", &self.codec)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fps", &self.fps)
            .finish()
    }
}

/// Tablet → PC: the tablet left the second screen.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenStopPayload {
    pub reason: ScreenStopReason,
}

/// Why the tablet stopped. Never gains a value (an older PC could not read
/// it): a new cause is sent as the closest one.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenStopReason {
    /// The user asked for the grid (long press on a volume key).
    User,
    /// The decoder refused the stream or failed.
    DecoderFailed,
    /// The stream connection could not be opened or was lost.
    StreamFailed,
}

/// PC → tablet: the second screen ended on the PC's side; back to the grid.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenStoppedPayload {
    pub reason: ScreenStoppedReason,
}

/// Why the PC stopped. Never gains a value (an older tablet could not read
/// it): a new cause is sent as the closest one, `failed` by default.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenStoppedReason {
    /// "Arrêter le second écran" on the PC, or the tablet's own `screen:stop`.
    Stopped,
    /// The user refused it on the PC (first-time confirmation, or the
    /// system's screen sharing dialog).
    Refused,
    /// Another device took the second screen.
    Replaced,
    /// Capture or encoding failed, or the ticket was not used in time.
    Failed,
}

/// Kind byte of a [`StreamFrame`] (its first byte).
mod kind {
    pub const VIDEO: u8 = 1;
    pub const ACK: u8 = 2;
    pub const KEY_FRAME_REQUEST: u8 = 3;
}

/// `flags` bit of a video frame: it starts a group (decodable alone).
const KEY_FRAME: u8 = 1;

/// A binary message of the stream connection. Byte layout (big-endian):
///
/// | Kind | Bytes |
/// |---|---|
/// | video (PC → tablet) | `01`, flags (`01` = key frame), capture time µs (u64), access unit |
/// | ack (tablet → PC) | `02`, capture time µs (u64) of the frame shown, decode time µs (u32) |
/// | key frame request (tablet → PC) | `03` |
///
/// Test vectors: `fixtures/vectors/stream.json` (Rust and Kotlin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamFrame<'a> {
    Video {
        key_frame: bool,
        /// When the picture was captured, on the PC's monotonic clock: only
        /// meaningful to the PC, echoed in the ack.
        capture_us: u64,
        data: &'a [u8],
    },
    /// The frame captured at `capture_us` was handed to the display.
    Ack { capture_us: u64, decode_us: u32 },
    /// The decoder lost the picture: send a key frame.
    KeyFrameRequest,
}

/// Why a stream message was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFrameError {
    Empty,
    /// A kind this side does not know (from a newer peer): skip it.
    UnknownKind(u8),
    /// Shorter than its kind's header.
    Truncated,
}

impl fmt::Display for StreamFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty stream message"),
            Self::UnknownKind(k) => write!(f, "unknown stream message kind {k}"),
            Self::Truncated => f.write_str("truncated stream message"),
        }
    }
}

impl std::error::Error for StreamFrameError {}

impl<'a> StreamFrame<'a> {
    /// The message's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Video {
                key_frame,
                capture_us,
                data,
            } => {
                let mut out = Vec::with_capacity(10 + data.len());
                out.push(kind::VIDEO);
                out.push(if *key_frame { KEY_FRAME } else { 0 });
                out.extend_from_slice(&capture_us.to_be_bytes());
                out.extend_from_slice(data);
                out
            }
            Self::Ack {
                capture_us,
                decode_us,
            } => {
                let mut out = Vec::with_capacity(13);
                out.push(kind::ACK);
                out.extend_from_slice(&capture_us.to_be_bytes());
                out.extend_from_slice(&decode_us.to_be_bytes());
                out
            }
            Self::KeyFrameRequest => vec![kind::KEY_FRAME_REQUEST],
        }
    }

    /// Reads a message. Bytes after an ack's or a request's fields are
    /// ignored (room for later fields).
    ///
    /// # Errors
    /// Empty, unknown kind, or shorter than the kind's header.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, StreamFrameError> {
        let (&kind, rest) = bytes.split_first().ok_or(StreamFrameError::Empty)?;
        match kind {
            kind::VIDEO => {
                let (&flags, rest) = rest.split_first().ok_or(StreamFrameError::Truncated)?;
                let (time, data) = rest
                    .split_first_chunk::<8>()
                    .ok_or(StreamFrameError::Truncated)?;
                Ok(Self::Video {
                    key_frame: flags & KEY_FRAME != 0,
                    capture_us: u64::from_be_bytes(*time),
                    data,
                })
            }
            kind::ACK => {
                let (time, rest) = rest
                    .split_first_chunk::<8>()
                    .ok_or(StreamFrameError::Truncated)?;
                let (decode, _) = rest
                    .split_first_chunk::<4>()
                    .ok_or(StreamFrameError::Truncated)?;
                Ok(Self::Ack {
                    capture_us: u64::from_be_bytes(*time),
                    decode_us: u32::from_be_bytes(*decode),
                })
            }
            kind::KEY_FRAME_REQUEST => Ok(Self::KeyFrameRequest),
            other => Err(StreamFrameError::UnknownKind(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let data = [0, 0, 0, 1, 0x67, 0x42];
        for frame in [
            StreamFrame::Video {
                key_frame: true,
                capture_us: 0x0102_0304_0506_0708,
                data: &data,
            },
            StreamFrame::Video {
                key_frame: false,
                capture_us: 1,
                data: &[],
            },
            StreamFrame::Ack {
                capture_us: u64::MAX,
                decode_us: 4242,
            },
            StreamFrame::KeyFrameRequest,
        ] {
            let bytes = frame.encode();
            assert_eq!(StreamFrame::decode(&bytes), Ok(frame));
        }
    }

    #[test]
    fn bad_frames_are_refused_without_panicking() {
        assert_eq!(StreamFrame::decode(&[]), Err(StreamFrameError::Empty));
        assert_eq!(
            StreamFrame::decode(&[9, 1, 2]),
            Err(StreamFrameError::UnknownKind(9))
        );
        for short in [
            &[1u8][..],
            &[1, 1, 0, 0, 0, 0, 0, 0, 0],
            &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ] {
            assert_eq!(
                StreamFrame::decode(short),
                Err(StreamFrameError::Truncated),
                "{short:?}"
            );
        }
    }

    #[test]
    fn later_fields_of_an_ack_are_ignored() {
        let mut bytes = StreamFrame::Ack {
            capture_us: 7,
            decode_us: 8,
        }
        .encode();
        bytes.extend_from_slice(&[0xff; 5]);
        assert_eq!(
            StreamFrame::decode(&bytes),
            Ok(StreamFrame::Ack {
                capture_us: 7,
                decode_us: 8
            })
        );
    }

    #[test]
    fn the_ticket_is_never_printed() {
        let offer = ScreenOfferPayload {
            ticket: "secret-ticket".into(),
            codec: VideoCodec::H264,
            width: 1728,
            height: 1080,
            fps: 60,
        };
        let printed = format!("{offer:?}");
        assert!(!printed.contains("secret-ticket"), "{printed}");
        assert!(printed.contains(REDACTED));
    }
}
