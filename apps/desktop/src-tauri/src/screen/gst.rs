//! Linux sources of the second screen, through the system's GStreamer: the
//! test pattern now, the screen (ScreenCast portal) next. Encoders: VA-API
//! H.264 (`vah264enc`, GStreamer 1.22+ with a VA driver that encodes), else
//! VP8 (`vp8enc`, libvpx; Ubuntu 22.04 has GStreamer 1.20, whose `va`
//! plugin only decodes). x264 is never used (GPL, patents).
//!
//! Frames are dropped **before** the encoder: a probe in front of it lets
//! a picture in only with a credit ([`ENCODER_AHEAD`] of them; one comes
//! back each time the stream takes a frame), so at most that many are in
//! the encoder or encoded and waiting for the network's window, and an
//! encoded frame is never thrown away (a missing P-frame breaks the
//! picture until the next key frame). Deciding per picture, as it
//! arrives, keeps the encoder fed (closing a valve between requests lost
//! pictures: H.264 stayed at ~49 frames/s). A thread pulls the encoded
//! frames into a channel, so `next_frame` can be cancelled without losing
//! one.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use padboard_protocol::VideoCodec;
use tokio::sync::mpsc;

use super::{ENCODER_AHEAD, EncodedFrame, FrameSource, StreamFormat};

/// Bitrate over USB (design: 12 Mbit/s; the Wi-Fi adaptation comes later).
const KBPS: u32 = 12_000;

/// GStreamer, initialised once; the error if it cannot be.
fn init() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| gst::init().map_err(|e| e.to_string()))
        .clone()
}

fn has(element: &str) -> bool {
    gst::ElementFactory::find(element).is_some()
}

/// Elements every Linux source needs, with the package that ships them
/// (Debian / Ubuntu names), for the message when one is missing.
const COMMON: [(&str, &str); 5] = [
    ("videotestsrc", "gstreamer1.0-plugins-base"),
    ("videoconvert", "gstreamer1.0-plugins-base"),
    ("timeoverlay", "gstreamer1.0-plugins-base"),
    ("appsink", "gstreamer1.0-plugins-base"),
    ("identity", "libgstreamer1.0-0"),
];

/// What the PC can encode here, best first; empty when GStreamer or an
/// element is missing (see [`missing`]).
#[must_use]
pub fn encoders() -> Vec<VideoCodec> {
    if init().is_err() || COMMON.iter().any(|(e, _)| !has(e)) {
        return Vec::new();
    }
    let mut codecs = Vec::new();
    if ["vapostproc", "vah264enc", "h264parse"]
        .iter()
        .all(|e| has(e))
    {
        codecs.push(VideoCodec::H264);
    }
    if has("vp8enc") {
        codecs.push(VideoCodec::Vp8);
    }
    codecs
}

/// Packages to install when no encoder is usable (for the message).
#[must_use]
pub fn missing() -> Vec<&'static str> {
    if init().is_err() {
        return vec!["libgstreamer1.0-0"];
    }
    let mut packages: Vec<&str> = COMMON
        .iter()
        .filter(|(e, _)| !has(e))
        .map(|&(_, p)| p)
        .collect();
    if !has("vp8enc") && !has("vah264enc") {
        packages.push("gstreamer1.0-plugins-good");
    }
    packages.dedup();
    packages
}

/// The pipeline of the test pattern: moving ball, running time in ms
/// (latency readable on a photo of both screens).
fn pattern_pipeline(format: StreamFormat) -> String {
    let (w, h, fps) = (format.width, format.height, format.fps);
    let src = format!(
        "videotestsrc is-live=true pattern=ball motion=sweep ! \
         video/x-raw,width={w},height={h},framerate={fps}/1 ! \
         timeoverlay font-desc=\"Sans 48\" time-mode=running-time"
    );
    let encode = match format.codec {
        VideoCodec::H264 => format!(
            "videoconvert ! vapostproc ! \
             video/x-raw(memory:VAMemory),format=NV12,width={w},height={h} ! \
             identity name=gate ! \
             vah264enc rate-control=cbr bitrate={KBPS} key-int-max={key} b-frames=0 target-usage=7 ! \
             video/x-h264,profile=main ! h264parse config-interval=-1 ! \
             video/x-h264,stream-format=byte-stream,alignment=au",
            key = fps * 2
        ),
        VideoCodec::Vp8 => format!(
            "videoconvert ! video/x-raw,format=I420 ! identity name=gate ! \
             vp8enc deadline=1 cpu-used=16 end-usage=cbr target-bitrate={bps} \
             keyframe-max-dist={key} lag-in-frames=0 threads=4 error-resilient=default",
            bps = KBPS * 1000,
            key = fps * 2
        ),
    };
    format!("{src} ! {encode} ! appsink name=sink sync=false max-buffers=0 drop=false")
}

/// The test pattern, encoded in `format`.
pub struct PatternSource {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    frames: mpsc::UnboundedReceiver<Option<EncodedFrame>>,
    /// Pictures that may still enter the encoder: [`ENCODER_AHEAD`] minus
    /// those in it or encoded and not taken yet.
    credits: Arc<AtomicUsize>,
}

impl PatternSource {
    /// Starts the pipeline; pictures enter the encoder while credits last.
    ///
    /// # Errors
    /// GStreamer or an element is missing, or the pipeline does not start.
    pub fn start(format: StreamFormat) -> Result<Self, String> {
        init()?;
        let pipeline = gst::parse::launch(&pattern_pipeline(format))
            .map_err(|e| e.to_string())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "not a pipeline".to_owned())?;
        let gate = pipeline.by_name("gate").ok_or("no gate")?;
        let sink = pipeline
            .by_name("sink")
            .ok_or("no sink")?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| "not an appsink".to_owned())?;
        // Each picture is let into the encoder only with a credit, decided
        // as it arrives: no window where a picture is dropped while the
        // stream is about to ask for one.
        let credits = Arc::new(AtomicUsize::new(ENCODER_AHEAD));
        let gate_credits = Arc::clone(&credits);
        gate.static_pad("src")
            .ok_or("no gate pad")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                let mut current = gate_credits.load(Ordering::Acquire);
                let taken = loop {
                    if current == 0 {
                        break false;
                    }
                    match gate_credits.compare_exchange_weak(
                        current,
                        current - 1,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break true,
                        Err(now) => current = now,
                    }
                };
                if taken {
                    gst::PadProbeReturn::Ok
                } else {
                    gst::PadProbeReturn::Drop
                }
            })
            .ok_or("cannot watch the gate")?;
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| e.to_string())?;
        let (frames_tx, frames) = mpsc::unbounded_channel();
        let pull = sink.clone();
        std::thread::Builder::new()
            .name("screen-pattern".into())
            .spawn(move || {
                // Until the pipeline stops (the source dropped) or fails.
                loop {
                    let frame = pull.pull_sample().ok().and_then(|s| encoded(&s));
                    let ended = frame.is_none();
                    if frames_tx.send(frame).is_err() || ended {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        tracing::info!(?format, "second screen: test pattern started");
        Ok(Self {
            pipeline,
            sink,
            frames,
            credits,
        })
    }
}

impl Drop for PatternSource {
    fn drop(&mut self) {
        // Ends the pulling thread (its pull_sample returns).
        let _ = self.pipeline.set_state(gst::State::Null);
        tracing::info!("second screen: test pattern stopped");
    }
}

fn encoded(sample: &gst::Sample) -> Option<EncodedFrame> {
    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    Some(EncodedFrame {
        key_frame: !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT),
        // Running time of the picture, as the overlay shows it.
        capture_us: buffer.pts().map_or(0, gst::ClockTime::useconds),
        encoded_at: std::time::Instant::now(),
        data: map.as_slice().to_vec(),
    })
}

impl FrameSource for PatternSource {
    fn next_frame(&mut self) -> Pin<Box<dyn Future<Output = Option<EncodedFrame>> + Send + '_>> {
        Box::pin(async move {
            // Cancel-safe: a frame left in the channel is taken next time.
            let frame = self.frames.recv().await.flatten()?;
            // Taken: one more picture may enter the encoder.
            self.credits.fetch_add(1, Ordering::AcqRel);
            Some(frame)
        })
    }

    fn request_key_frame(&mut self) {
        let event = gst::event::CustomUpstream::new(
            gst::Structure::builder("GstForceKeyUnit")
                .field("all-headers", true)
                .build(),
        );
        self.sink.send_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(codec: VideoCodec) -> StreamFormat {
        StreamFormat {
            codec,
            width: 320,
            height: 200,
            fps: 60,
        }
    }

    #[test]
    fn the_pipelines_name_their_gate_and_sink() {
        for codec in [VideoCodec::H264, VideoCodec::Vp8] {
            let p = pattern_pipeline(format(codec));
            assert!(p.contains("identity name=gate"), "{p}");
            assert!(p.contains("appsink name=sink"), "{p}");
            assert!(!p.contains("x264"), "{p}");
        }
    }

    /// Needs GStreamer with vp8enc (on CI and the development PC): the
    /// pattern encodes, starts with a key frame, and gives one frame per
    /// request only.
    #[tokio::test]
    async fn the_pattern_gives_vp8_frames_on_request() {
        if !encoders().contains(&VideoCodec::Vp8) {
            eprintln!("SKIPPED: no vp8enc here (missing: {:?})", missing());
            return;
        }
        let mut source = PatternSource::start(format(VideoCodec::Vp8)).unwrap();
        let first = source.next_frame().await.unwrap();
        assert!(first.key_frame);
        assert!(!first.data.is_empty());
        let second = source.next_frame().await.unwrap();
        assert!(second.capture_us > first.capture_us);
        // At most ENCODER_AHEAD frames are encoded ahead of the stream.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let mut waiting = 0;
        while source.frames.try_recv().is_ok() {
            waiting += 1;
        }
        assert!(waiting <= ENCODER_AHEAD, "{waiting} frames encoded ahead");
        // Taken by hand: their credits come back as next_frame would.
        source.credits.fetch_add(waiting, Ordering::AcqRel);
        source.request_key_frame();
        let mut key = false;
        for _ in 0..5 {
            key |= source.next_frame().await.unwrap().key_frame;
        }
        assert!(key, "no key frame after the request");
    }

    /// Frames per second the stream gets from the pattern with a window of
    /// two unacknowledged frames and acks `ack` after each send (the
    /// SM-T580 measured ~20 ms). Run by hand:
    /// `cargo test -p padboard-desktop --lib throughput -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "a measurement, several seconds"]
    async fn throughput() {
        use std::collections::VecDeque;
        use std::time::{Duration, Instant};
        let ack = Duration::from_millis(
            std::env::var("PADBOARD_BENCH_ACK_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
        );
        for codec in encoders() {
            let mut source = PatternSource::start(StreamFormat {
                codec,
                width: 1728,
                height: 1080,
                fps: 60,
            })
            .unwrap();
            let mut flow = crate::screen::flow::Flow::default();
            let mut acks: VecDeque<(Instant, u64)> = VecDeque::new();
            let start = Instant::now();
            let mut frames = 0u32;
            let mut waits: Vec<f64> = Vec::new();
            while start.elapsed() < Duration::from_secs(5) {
                while acks.front().is_some_and(|&(at, _)| at <= Instant::now()) {
                    let (_, t) = acks.pop_front().unwrap();
                    flow.acked(t);
                }
                if !flow.can_send() {
                    let (at, _) = *acks.front().unwrap();
                    tokio::time::sleep_until(at.into()).await;
                    continue;
                }
                let frame = source.next_frame().await.unwrap();
                flow.sent(frame.capture_us);
                waits.push(frame.encoded_at.elapsed().as_secs_f64() * 1000.0);
                acks.push_back((Instant::now() + ack, frame.capture_us));
                frames += 1;
            }
            let fps = f64::from(frames) / start.elapsed().as_secs_f64();
            waits.sort_by(f64::total_cmp);
            let wait = |q: f64| waits[((waits.len() - 1) as f64 * q) as usize];
            eprintln!(
                "THROUGHPUT {codec:?}: {fps:.1} fps (acks after {ack:?}), \
                 encoded → sent p50 {:.1} ms, p90 {:.1} ms",
                wait(0.5),
                wait(0.9)
            );
        }
    }

    /// Needs a VA driver that encodes H.264 (this development PC; not CI):
    /// Annex B access units, the parameter sets (SPS, NAL type 7) before
    /// the key frame.
    #[tokio::test]
    async fn the_pattern_gives_h264_frames_when_va_encodes() {
        if !encoders().contains(&VideoCodec::H264) {
            eprintln!("SKIPPED: no VA-API H.264 encoder here");
            return;
        }
        let mut source = PatternSource::start(format(VideoCodec::H264)).unwrap();
        let first = source.next_frame().await.unwrap();
        assert!(first.key_frame);
        assert!(
            first.data.starts_with(&[0, 0, 0, 1]),
            "{:?}",
            &first.data[..8]
        );
        assert!(
            first
                .data
                .windows(5)
                .any(|w| w[..4] == [0, 0, 0, 1] && w[4] & 0x1f == 7),
            "no SPS before the key frame"
        );
        assert!(!source.next_frame().await.unwrap().key_frame);
    }
}
