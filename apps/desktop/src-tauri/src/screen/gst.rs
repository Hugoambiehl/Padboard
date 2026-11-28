//! Linux sources of the second screen, through the system's GStreamer: the
//! screen (ScreenCast portal, `pipewiresrc`) and the test pattern. Encoders: VA-API
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
const COMMON: [(&str, &str); 6] = [
    ("pipewiresrc", "gstreamer1.0-pipewire"),
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
    // Tests: only these encoders (`vp8`: a PC without VA-API H.264, as
    // Ubuntu 22.04), e.g. PADBOARD_SCREEN_ENCODERS=vp8.
    if let Ok(only) = std::env::var("PADBOARD_SCREEN_ENCODERS") {
        codecs.retain(|c| only.split(',').any(|o| o.trim() == codec_name(*c)));
    }
    codecs
}

fn codec_name(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::Vp8 => "vp8",
    }
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

/// Where the pictures come from.
pub enum Input {
    /// Moving ball, running time in ms (latency readable on a photo of
    /// both screens).
    Pattern,
    /// A ScreenCast portal session, kept open while the source runs.
    Screen(Arc<super::portal::Session>),
}

/// The pipeline reading `input` and encoding it in `format`.
fn pipeline(input: &Input, format: StreamFormat) -> String {
    use std::os::fd::AsRawFd;
    let (w, h, fps) = (format.width, format.height, format.fps);
    let src = match input {
        Input::Pattern => format!(
            "videotestsrc is-live=true pattern=ball motion=sweep ! \
             video/x-raw,width={w},height={h},framerate={fps}/1 ! \
             timeoverlay font-desc=\"Sans 48\" time-mode=running-time"
        ),
        // A virtual monitor takes the size the caps ask for; a picture
        // comes when the screen changes (`keepalive-time` sends the last
        // one again meanwhile, set in `start`), at most `fps`.
        Input::Screen(session) => format!(
            "pipewiresrc name=src fd={fd} path={node} do-timestamp=true ! \
             video/x-raw,width={w},height={h},max-framerate={fps}/1",
            fd = session.fd.as_raw_fd(),
            node = session.node,
        ),
    };
    let encode = match format.codec {
        // vapostproc converts on the GPU (the screen's BGRx included).
        VideoCodec::H264 => format!(
            "{convert}vapostproc ! \
             video/x-raw(memory:VAMemory),format=NV12,width={w},height={h} ! \
             identity name=gate ! \
             vah264enc rate-control=cbr bitrate={KBPS} key-int-max={key} b-frames=0 target-usage=7 ! \
             video/x-h264,profile=main ! h264parse config-interval=-1 ! \
             video/x-h264,stream-format=byte-stream,alignment=au",
            convert = if matches!(input, Input::Pattern) {
                "videoconvert ! "
            } else {
                ""
            },
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

/// A picture source encoded in `format`.
pub struct GstSource {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    frames: mpsc::UnboundedReceiver<Option<EncodedFrame>>,
    /// Pictures that may still enter the encoder: [`ENCODER_AHEAD`] minus
    /// those in it or encoded and not taken yet.
    credits: Arc<AtomicUsize>,
    /// Closed (a virtual monitor removed) once the pipeline has stopped.
    input: Input,
}

impl GstSource {
    /// Starts the pipeline; pictures enter the encoder while credits last.
    ///
    /// # Errors
    /// GStreamer or an element is missing, or the pipeline does not start.
    pub fn start(input: Input, format: StreamFormat) -> Result<Self, String> {
        init()?;
        let pipeline = gst::parse::launch(&pipeline(&input, format))
            .map_err(|e| e.to_string())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "not a pipeline".to_owned())?;
        if let Some(src) = pipeline.by_name("src")
            && src.find_property("keepalive-time").is_some()
        {
            // A still screen sends no picture: a key frame asked for then
            // comes with the last one, sent again.
            src.set_property("keepalive-time", 500_i32);
        }
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
            .name("screen-source".into())
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
        tracing::info!(
            ?format,
            input = input.name(),
            "second screen: source started"
        );
        Ok(Self {
            pipeline,
            sink,
            frames,
            credits,
            input,
        })
    }
}

impl Input {
    fn name(&self) -> &'static str {
        match self {
            Self::Pattern => "test pattern",
            Self::Screen(_) => "screen",
        }
    }
}

impl Drop for GstSource {
    fn drop(&mut self) {
        // Ends the pulling thread (its pull_sample returns); the portal
        // session (field `input`) is closed after.
        let _ = self.pipeline.set_state(gst::State::Null);
        tracing::info!(input = self.input.name(), "second screen: source stopped");
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

impl FrameSource for GstSource {
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
            let p = pipeline(&Input::Pattern, format(codec));
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
        let mut source = GstSource::start(Input::Pattern, format(VideoCodec::Vp8)).unwrap();
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

    /// Streams `source` for `secs` with a window of two unacknowledged
    /// frames, each acked `ack` after it is sent: frames per second and the
    /// sorted waits between encoded and sent (ms).
    async fn stream_for(
        source: &mut GstSource,
        ack: std::time::Duration,
        secs: u64,
    ) -> (f64, Vec<f64>) {
        use std::collections::VecDeque;
        use std::time::{Duration, Instant};
        let mut flow = crate::screen::flow::Flow::default();
        let mut acks: VecDeque<(Instant, u64)> = VecDeque::new();
        let start = Instant::now();
        let end = start + Duration::from_secs(secs);
        let mut frames = 0u32;
        let mut waits: Vec<f64> = Vec::new();
        while Instant::now() < end {
            while acks.front().is_some_and(|&(at, _)| at <= Instant::now()) {
                let (_, t) = acks.pop_front().unwrap();
                flow.acked(t);
            }
            if !flow.can_send() {
                let (at, _) = *acks.front().unwrap();
                tokio::time::sleep_until(at.into()).await;
                continue;
            }
            // A still screen sends nothing: give up at the end.
            let Ok(frame) = tokio::time::timeout_at(end.into(), source.next_frame()).await else {
                break;
            };
            let frame = frame.unwrap();
            flow.sent(frame.capture_us);
            waits.push(frame.encoded_at.elapsed().as_secs_f64() * 1000.0);
            acks.push_back((Instant::now() + ack, frame.capture_us));
            frames += 1;
        }
        waits.sort_by(f64::total_cmp);
        (f64::from(frames) / start.elapsed().as_secs_f64(), waits)
    }

    fn bench_ack() -> std::time::Duration {
        std::time::Duration::from_millis(
            std::env::var("PADBOARD_BENCH_ACK_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
        )
    }

    fn quantile(sorted: &[f64], q: f64) -> f64 {
        if sorted.is_empty() {
            return f64::NAN;
        }
        sorted[((sorted.len() - 1) as f64 * q) as usize]
    }

    /// Frames per second the stream gets from the pattern with a window of
    /// two unacknowledged frames and acks `ack` after each send (the
    /// SM-T580 measured ~20 ms). Run by hand:
    /// `cargo test -p padboard-desktop --lib throughput -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "a measurement, several seconds"]
    async fn throughput() {
        let ack = bench_ack();
        for codec in encoders() {
            let mut source = GstSource::start(
                Input::Pattern,
                StreamFormat {
                    codec,
                    width: 1728,
                    height: 1080,
                    fps: 60,
                },
            )
            .unwrap();
            let (fps, waits) = stream_for(&mut source, ack, 5).await;
            eprintln!(
                "THROUGHPUT {codec:?}: {fps:.1} fps (acks after {ack:?}), \
                 encoded → sent p50 {:.1} ms, p90 {:.1} ms",
                quantile(&waits, 0.5),
                quantile(&waits, 0.9)
            );
        }
    }

    /// CPU time (s) of process `pid` so far, from `/proc` (USER_HZ = 100).
    fn cpu_seconds(pid: &str) -> f64 {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        // Fields after the command name (which may hold spaces).
        let fields: Vec<&str> = stat
            .rsplit_once(')')
            .map(|(_, rest)| rest.split_whitespace().collect())
            .unwrap_or_default();
        let ticks = |i: usize| {
            fields
                .get(i)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        // utime and stime: fields 14 and 15, 12 and 13 after the name.
        (ticks(11) + ticks(12)) / 100.0
    }

    /// The compositor's process (GNOME), which captures the screen.
    fn compositor_pid() -> Option<String> {
        std::fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
            let pid = e.file_name().into_string().ok()?;
            let comm = std::fs::read_to_string(e.path().join("comm")).ok()?;
            (comm.trim() == "gnome-shell" && pid.parse::<u32>().is_ok()).then_some(pid)
        })
    }

    /// The CPU cost of the screen (a virtual monitor of the SM-T580's
    /// stream size) per encoder: Padboard's process and the compositor's,
    /// in % of one core. Asks the desktop to share a screen (a dialog on
    /// the PC); move something that changes (a video) onto the new screen
    /// before the measurement starts. Run by hand:
    /// `cargo test -p padboard-desktop --lib screen_cost -- --ignored --nocapture`
    /// (`PADBOARD_BENCH_SECS`, default 20 per encoder).
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "asks to share the screen, a measurement"]
    async fn screen_cost() {
        let secs: u64 = std::env::var("PADBOARD_BENCH_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        let session = Arc::new(
            tokio::task::spawn_blocking(|| super::super::portal::start(None))
                .await
                .unwrap()
                .unwrap(),
        );
        let compositor = compositor_pid();
        let format = |codec| StreamFormat {
            codec,
            width: 1728,
            height: 1080,
            fps: 60,
        };
        // The virtual monitor exists once a stream reads it.
        let warm_up =
            GstSource::start(Input::Screen(Arc::clone(&session)), format(VideoCodec::Vp8));
        eprintln!("SCREEN: move a video onto the new screen; measuring in 45 s");
        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
        drop(warm_up);
        for codec in encoders() {
            let mut source =
                GstSource::start(Input::Screen(Arc::clone(&session)), format(codec)).unwrap();
            // Pictures the compositor gives, before the gate.
            let raw = Arc::new(AtomicUsize::new(0));
            let counted = Arc::clone(&raw);
            let src_pad = source
                .pipeline
                .by_name("src")
                .and_then(|src| src.static_pad("src"))
                .unwrap();
            src_pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                counted.fetch_add(1, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            });
            // Negotiation and the first key frame out of the measurement.
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(3), source.next_frame()).await;
            eprintln!(
                "SCREEN {codec:?} caps: {:?}",
                src_pad.current_caps().map(|c| c.to_string())
            );
            raw.store(0, Ordering::Relaxed);
            let ours = cpu_seconds("self");
            let theirs = compositor.as_deref().map(cpu_seconds);
            let start = std::time::Instant::now();
            let (fps, waits) = stream_for(&mut source, bench_ack(), secs).await;
            let elapsed = start.elapsed().as_secs_f64();
            let percent = |before: f64, after: f64| (after - before) / elapsed * 100.0;
            eprintln!(
                "SCREEN {codec:?}: compositor gave {:.1} fps",
                raw.load(Ordering::Relaxed) as f64 / elapsed
            );
            eprintln!(
                "SCREEN {codec:?}: {fps:.1} fps, Padboard {:.0} % of a core, compositor {}, \
                 encoded → sent p50 {:.1} ms",
                percent(ours, cpu_seconds("self")),
                match (compositor.as_deref(), theirs) {
                    (Some(pid), Some(before)) =>
                        format!("{:.0} %", percent(before, cpu_seconds(pid))),
                    _ => "not found".into(),
                },
                quantile(&waits, 0.5),
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
        let mut source = GstSource::start(Input::Pattern, format(VideoCodec::H264)).unwrap();
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
