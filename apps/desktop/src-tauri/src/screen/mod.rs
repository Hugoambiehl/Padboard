//! Second screen (#20, `docs/design/second-screen.md`): the PC streams a
//! picture to the tablet on a second WebSocket (`/stream`, same port and
//! transport), opened with a one-time ticket from `screen:offer`.
//!
//! - [`format`]: the stream's codec and size, from the tablet's capability.
//! - [`ticket`]: one-time tickets.
//! - [`flow`]: at most two frames in flight.
//! - [`stream`]: the loop of a stream connection, pulling frames from a
//!   [`FrameSource`] only while the flow allows it.
//!
//! One stream at a time on the PC: a new offer replaces the running one.

pub mod flow;
pub mod format;
#[cfg(target_os = "linux")]
pub mod gst;
pub mod stream;
pub mod ticket;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use padboard_protocol::VideoCodec;
use tokio::sync::watch;

pub use format::StreamFormat;

use crate::auth::RandomError;
use ticket::Tickets;

/// An encoded picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub key_frame: bool,
    /// When it was captured, on the PC's monotonic clock (µs).
    pub capture_us: u64,
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
    /// The next picture; `None` when the source ended (capture lost).
    fn next_frame(&mut self) -> Pin<Box<dyn Future<Output = Option<EncodedFrame>> + Send + '_>>;
    /// The tablet's decoder lost the picture: the next frame is a key frame.
    fn request_key_frame(&mut self);
}

/// A kind of source this PC has (the test pattern for now; the screen
/// next), with what it can encode here.
#[derive(Debug, Clone, Copy)]
pub struct SourceFactory {
    /// For the log.
    pub name: &'static str,
    /// Codecs it can produce on this PC, best first (empty: missing pieces).
    pub encoders: fn() -> Vec<VideoCodec>,
    /// Packages to install when `encoders` is empty.
    pub missing: fn() -> Vec<&'static str>,
    /// Starts a source for a stream.
    pub start: fn(StreamFormat) -> Result<Box<dyn FrameSource>, String>,
}

/// The test pattern (Linux: GStreamer), for development and tests until
/// the screen capture: `PADBOARD_SCREEN_SOURCE=pattern`.
#[cfg(target_os = "linux")]
#[must_use]
pub fn pattern_factory() -> SourceFactory {
    SourceFactory {
        name: "test pattern",
        encoders: gst::encoders,
        missing: gst::missing,
        start: |format| {
            gst::PatternSource::start(format).map(|s| Box::new(s) as Box<dyn FrameSource>)
        },
    }
}

/// The source the second screen uses on this PC, if any.
#[must_use]
pub fn factory() -> Option<SourceFactory> {
    #[cfg(target_os = "linux")]
    if std::env::var("PADBOARD_SCREEN_SOURCE").is_ok_and(|v| v == "pattern") {
        return Some(pattern_factory());
    }
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
            start: |_| {
                Ok(Box::new(FakeSource {
                    interval: Duration::from_millis(5),
                    ..FakeSource::default()
                }))
            },
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
