//! Second screen (#20, `docs/design/second-screen.md`): the PC streams a
//! picture to the tablet on a second WebSocket (`/stream`, same port and
//! transport), opened with a one-time ticket from `screen:offer`.
//!
//! - [`format`]: the stream's codec and size, from the tablet's capability.
//! - [`ticket`]: one-time tickets.
//! - [`flow`]: at most two frames in flight.
//! - [`gst`], [`portal`] (Linux): the screen and the test pattern.
//! - [`stream`]: the loop of a stream connection, pulling frames from a
//!   [`FrameSource`] only while the flow allows it.
//!
//! One stream at a time on the PC: a new offer replaces the running one.

pub mod flow;
pub mod format;
#[cfg(target_os = "linux")]
pub mod gst;
#[cfg(target_os = "linux")]
pub mod portal;
pub mod stream;
pub mod ticket;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use padboard_protocol::VideoCodec;
use tokio::sync::watch;

pub use format::StreamFormat;

use crate::auth::RandomError;
use ticket::Tickets;

/// Pictures a source keeps in its encoder at once, besides the network's
/// window of unacknowledged frames ([`flow::WINDOW`]). One at a time left a
/// VA-API H.264 encoder at ~49 frames/s on the SM-T580 (its latency added
/// to each picture's wait).
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "only the GStreamer source exists so far")
)]
pub const ENCODER_AHEAD: usize = 2;

/// An encoded picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub key_frame: bool,
    /// When it was captured, on the PC's monotonic clock (µs).
    pub capture_us: u64,
    /// When the encoder gave it (how long it then waits for the network
    /// is logged).
    pub encoded_at: std::time::Instant,
    /// One access unit (H.264 Annex B, or one VP8 frame).
    pub data: Vec<u8>,
}

/// Where a stream's pictures come from: the screen (Windows, Linux) or the
/// test pattern.
///
/// The stream asks for a frame only while the tablet can take one: a
/// source must not queue frames meanwhile (keep at most the latest
/// captured picture, encode it when asked), so pictures are dropped before
/// the encoder. `next_frame` must be cancel-safe: the stream drops it when
/// something else happens first.
pub trait FrameSource: Send {
    /// The next picture; `None` when the source ended (capture lost). A
    /// source keeps up to [`ENCODER_AHEAD`] pictures in its encoder once
    /// asked, so its latency overlaps the next picture's wait.
    fn next_frame(&mut self) -> Pin<Box<dyn Future<Output = Option<EncodedFrame>> + Send + '_>>;
    /// The tablet's decoder lost the picture: the next frame is a key frame.
    fn request_key_frame(&mut self);
}

/// Starts a source for a stream (may block: the PC's user may be asked).
pub type StartSource =
    Arc<dyn Fn(StreamFormat) -> Result<Box<dyn FrameSource>, String> + Send + Sync>;

/// A kind of source this PC has (the screen, or the test pattern), with
/// what it can encode here.
#[derive(Clone)]
pub struct SourceFactory {
    /// For the log.
    pub name: &'static str,
    /// Codecs it can produce on this PC, best first (empty: missing pieces).
    pub encoders: fn() -> Vec<VideoCodec>,
    /// Packages to install when `encoders` is empty.
    pub missing: fn() -> Vec<&'static str>,
    /// Starts a source for a stream.
    pub start: StartSource,
}

impl fmt::Debug for SourceFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceFactory")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The test pattern (Linux: GStreamer), for development and tests:
/// `PADBOARD_SCREEN_SOURCE=pattern`.
#[cfg(target_os = "linux")]
#[must_use]
pub fn pattern_factory() -> SourceFactory {
    SourceFactory {
        name: "test pattern",
        encoders: gst::encoders,
        missing: gst::missing,
        start: Arc::new(|format| {
            gst::GstSource::start(gst::Input::Pattern, format)
                .map(|s| Box::new(s) as Box<dyn FrameSource>)
        }),
    }
}

/// File keeping the ScreenCast portal's restore token (owner-only): the
/// PC's user is asked once, until they revoke it in the desktop's settings.
#[cfg(target_os = "linux")]
const RESTORE_TOKEN_FILE: &str = "screen-cast-token";

/// The screen (Linux: a virtual monitor through the ScreenCast portal).
#[cfg(target_os = "linux")]
#[must_use]
pub fn screen_factory(config: crate::storage::ConfigDir) -> SourceFactory {
    SourceFactory {
        name: "screen",
        encoders: gst::encoders,
        missing: gst::missing,
        start: Arc::new(move |format| {
            let restore = config
                .read_raw(RESTORE_TOKEN_FILE)
                .ok()
                .flatten()
                .and_then(|bytes| String::from_utf8(bytes).ok());
            let session = portal::start(restore.as_deref()).map_err(|e| e.to_string())?;
            // Each token is good once: keep the new one.
            if let Some(token) = &session.restore_token
                && let Err(e) = config.write_secret(RESTORE_TOKEN_FILE, token.as_bytes())
            {
                tracing::warn!(error = %e, "second screen: cannot keep the sharing permission");
            }
            gst::GstSource::start(gst::Input::Screen(Arc::new(session)), format)
                .map(|s| Box::new(s) as Box<dyn FrameSource>)
        }),
    }
}

/// The source the second screen uses on this PC, if any: for now only
/// on request (`PADBOARD_SCREEN_SOURCE=screen` or `pattern`, Linux), until
/// the tablet shows it.
#[must_use]
pub fn factory(config: &crate::storage::ConfigDir) -> Option<SourceFactory> {
    #[cfg(target_os = "linux")]
    match std::env::var("PADBOARD_SCREEN_SOURCE").as_deref() {
        Ok("pattern") => return Some(pattern_factory()),
        Ok("screen") => return Some(screen_factory(config.clone())),
        _ => {}
    }
    #[cfg(not(target_os = "linux"))]
    let _ = config;
    None
}

/// What a ticket stands for: a stream ready to be served.
pub struct Job {
    pub client_id: u64,
    pub format: StreamFormat,
    pub source: Box<dyn FrameSource>,
    /// Set when the stream must stop (replaced, stopped on the PC or by
    /// the tablet).
    pub stop: watch::Receiver<bool>,
    serial: u64,
}

impl Job {
    /// Identifies this offer among the PC's streams.
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job")
            .field("client_id", &self.client_id)
            .field("format", &self.format)
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct Active {
    client_id: u64,
    serial: u64,
    stop: watch::Sender<bool>,
}

#[derive(Debug, Default)]
struct Inner {
    tickets: Tickets<Job>,
    active: Option<Active>,
    next_serial: u64,
}

/// The PC's second screen: the pending offer or the running stream.
#[derive(Debug, Default)]
pub struct Screens {
    inner: Mutex<Inner>,
    /// A source is being started (the PC's user may be answering the
    /// desktop's dialog): further presses are ignored meanwhile.
    starting: std::sync::atomic::AtomicBool,
}

/// Held while a source starts ([`Screens::starting`]).
#[derive(Debug)]
pub struct Starting<'a>(&'a std::sync::atomic::AtomicBool);

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

/// A new offer, ready to send.
#[derive(Debug)]
pub struct Offered {
    pub ticket: String,
    pub serial: u64,
    /// The tablet whose stream this one replaces (told `replaced`).
    pub replaced: Option<u64>,
}

impl Screens {
    /// Marks a source as starting until the guard is dropped; `None` if
    /// one already is.
    pub fn starting(&self) -> Option<Starting<'_>> {
        use std::sync::atomic::Ordering;
        self.starting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Starting(&self.starting))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a stream for `client_id`, replacing the running one.
    ///
    /// # Errors
    /// The OS random generator failed (no ticket).
    pub fn offer(
        &self,
        client_id: u64,
        format: StreamFormat,
        source: Box<dyn FrameSource>,
        now: Instant,
    ) -> Result<Offered, RandomError> {
        let mut inner = self.lock();
        let replaced = inner.active.take().map(|old| {
            let _ = old.stop.send(true);
            old.client_id
        });
        let previous = replaced;
        inner.tickets.retain(|job| Some(job.client_id) != previous);
        inner.next_serial += 1;
        let serial = inner.next_serial;
        let (stop_tx, stop) = watch::channel(false);
        let job = Job {
            client_id,
            format,
            source,
            stop,
            serial,
        };
        let ticket = inner.tickets.issue(job, now)?;
        inner.active = Some(Active {
            client_id,
            serial,
            stop: stop_tx,
        });
        Ok(Offered {
            ticket,
            serial,
            replaced: replaced.filter(|&old| old != client_id),
        })
    }

    /// The stream a ticket stands for (once, while valid).
    pub fn redeem(&self, ticket: &str, now: Instant) -> Option<Job> {
        let mut inner = self.lock();
        let job = inner.tickets.redeem(ticket, now)?;
        // Replaced meanwhile: its stop is already set, served for nothing.
        inner
            .active
            .as_ref()
            .is_some_and(|a| a.serial == job.serial)
            .then_some(job)
    }

    /// The ticket was not used in time: the offer is dropped. Returns its
    /// tablet if that offer was still the current one.
    pub fn expire(&self, ticket: &str, serial: u64) -> Option<u64> {
        let mut inner = self.lock();
        let job = inner.tickets.take(ticket)?;
        if inner.active.as_ref().is_some_and(|a| a.serial == serial) {
            inner.active = None;
        }
        Some(job.client_id)
    }

    /// Stops the stream of `client_id` (any tablet's with `None`): the PC's
    /// "Arrêter", the tablet's `screen:stop`, or its session ending.
    /// Returns the tablet whose stream was stopped.
    pub fn stop(&self, client_id: Option<u64>) -> Option<u64> {
        let mut inner = self.lock();
        let active = inner
            .active
            .take_if(|a| client_id.is_none_or(|id| id == a.client_id))?;
        let _ = active.stop.send(true);
        inner
            .tickets
            .retain(|job| job.client_id != active.client_id);
        Some(active.client_id)
    }

    /// A stream connection ended (whatever the reason).
    pub fn ended(&self, serial: u64) {
        let mut inner = self.lock();
        if inner.active.as_ref().is_some_and(|a| a.serial == serial) {
            inner.active = None;
        }
    }

    /// The tablet showing the second screen, if any.
    #[cfg(test)]
    #[must_use]
    pub fn active(&self) -> Option<u64> {
        self.lock().active.as_ref().map(|a| a.client_id)
    }
}

#[cfg(test)]
pub mod testing {
    //! A source of numbered fake frames (not a real codec), for tests.

    use std::time::Duration;

    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeSource {
        pub sent: u64,
        pub key_frames_asked: u64,
        /// Frames before the source ends (`None`: never).
        pub ends_after: Option<u64>,
        pub interval: Duration,
    }

    /// A factory of fake sources (H.264 only).
    #[must_use]
    pub fn fake_factory() -> SourceFactory {
        SourceFactory {
            name: "fake",
            encoders: || vec![VideoCodec::H264],
            missing: Vec::new,
            start: Arc::new(|_| {
                Ok(Box::new(FakeSource {
                    interval: Duration::from_millis(5),
                    ..FakeSource::default()
                }) as Box<dyn FrameSource>)
            }),
        }
    }

    impl FrameSource for FakeSource {
        fn next_frame(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Option<EncodedFrame>> + Send + '_>> {
            Box::pin(async move {
                if self.ends_after.is_some_and(|n| self.sent >= n) {
                    return None;
                }
                tokio::time::sleep(self.interval).await;
                self.sent += 1;
                Some(EncodedFrame {
                    key_frame: self.sent == 1,
                    capture_us: self.sent * 1000,
                    encoded_at: std::time::Instant::now(),
                    data: vec![0, 0, 0, 1, u8::try_from(self.sent % 256).unwrap_or(0)],
                })
            })
        }

        fn request_key_frame(&mut self) {
            self.key_frames_asked += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::FakeSource;
    use super::*;

    #[test]
    fn one_source_starts_at_a_time() {
        let screens = Screens::default();
        let first = screens.starting().unwrap();
        // The PC's dialog is open: another press is ignored.
        assert!(screens.starting().is_none());
        drop(first);
        assert!(screens.starting().is_some());
    }

    fn format() -> StreamFormat {
        StreamFormat {
            codec: VideoCodec::H264,
            width: 1728,
            height: 1080,
            fps: 60,
        }
    }

    #[test]
    fn an_offer_is_served_once_then_stopped() {
        let screens = Screens::default();
        let now = Instant::now();
        let offered = screens
            .offer(1, format(), Box::new(FakeSource::default()), now)
            .unwrap();
        assert_eq!(offered.replaced, None);
        let job = screens.redeem(&offered.ticket, now).unwrap();
        assert_eq!(job.client_id, 1);
        assert!(screens.redeem(&offered.ticket, now).is_none());
        assert_eq!(screens.active(), Some(1));
        assert_eq!(screens.stop(Some(2)), None);
        assert_eq!(screens.stop(Some(1)), Some(1));
        assert!(*job.stop.borrow());
        assert_eq!(screens.active(), None);
    }

    #[test]
    fn a_new_offer_replaces_the_running_stream() {
        let screens = Screens::default();
        let now = Instant::now();
        let first = screens
            .offer(1, format(), Box::new(FakeSource::default()), now)
            .unwrap();
        let job = screens.redeem(&first.ticket, now).unwrap();
        let second = screens
            .offer(2, format(), Box::new(FakeSource::default()), now)
            .unwrap();
        assert_eq!(second.replaced, Some(1));
        assert!(*job.stop.borrow());
        // The first stream ending does not end the second.
        screens.ended(job.serial());
        assert_eq!(screens.active(), Some(2));
    }

    #[test]
    fn an_unused_offer_expires_and_a_replaced_ticket_is_dead() {
        let screens = Screens::default();
        let now = Instant::now();
        let first = screens
            .offer(1, format(), Box::new(FakeSource::default()), now)
            .unwrap();
        let second = screens
            .offer(2, format(), Box::new(FakeSource::default()), now)
            .unwrap();
        assert!(screens.redeem(&first.ticket, now).is_none());
        assert_eq!(screens.expire(&second.ticket, second.serial), Some(2));
        assert_eq!(screens.active(), None);
        assert_eq!(screens.expire(&second.ticket, second.serial), None);
    }
}
