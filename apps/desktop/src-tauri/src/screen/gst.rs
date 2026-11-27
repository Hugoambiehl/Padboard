//! Linux sources of the second screen, through the system's GStreamer: the
//! test pattern now, the screen (ScreenCast portal) next. Encoders: VA-API
//! H.264 (`vah264enc`, GStreamer 1.22+ with a VA driver that encodes), else
//! VP8 (`vp8enc`, libvpx; Ubuntu 22.04 has GStreamer 1.20, whose `va`
//! plugin only decodes). x264 is never used (GPL, patents).
//!
//! Frames are dropped **before** the encoder: a `valve` in front of it
//! opens only when the stream asks for a picture, and closes as soon as
//! one encoded frame came out, so an encoded frame is never thrown away
//! (a missing P-frame breaks the picture until the next key frame). A
//! thread owns the pipeline; the stream asks it for frames through a
//! channel, so `next_frame` can be cancelled without losing one.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::sync::mpsc as std_mpsc;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use padboard_protocol::VideoCodec;
use tokio::sync::mpsc;

use super::{EncodedFrame, FrameSource, StreamFormat};

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
    ("valve", "libgstreamer1.0-0"),
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
             valve name=gate drop=true ! \
             vah264enc rate-control=cbr bitrate={KBPS} key-int-max={key} b-frames=0 target-usage=7 ! \
             video/x-h264,profile=main ! h264parse config-interval=-1 ! \
             video/x-h264,stream-format=byte-stream,alignment=au",
            key = fps * 2
        ),
        VideoCodec::Vp8 => format!(
            "videoconvert ! video/x-raw,format=I420 ! valve name=gate drop=true ! \
             vp8enc deadline=1 cpu-used=16 end-usage=cbr target-bitrate={bps} \
             keyframe-max-dist={key} lag-in-frames=0 threads=4 error-resilient=default",
            bps = KBPS * 1000,
            key = fps * 2
        ),
    };
    format!("{src} ! {encode} ! appsink name=sink sync=false max-buffers=0 drop=false")
}

enum Request {
    Frame,
    KeyFrame,
}

/// The test pattern, encoded in `format`.
pub struct PatternSource {
    requests: std_mpsc::Sender<Request>,
    frames: mpsc::UnboundedReceiver<Option<EncodedFrame>>,
    asked: bool,
}

impl PatternSource {
    /// Starts the pipeline (paused at the valve until a frame is asked).
    ///
    /// # Errors
    /// GStreamer or an element is missing, or the pipeline does not start.
    pub fn start(format: StreamFormat) -> Result<Self, String> {
        init()?;
        let pipeline = gst::parse::launch(&pattern_pipeline(format))
            .map_err(|e| e.to_string())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "not a pipeline".to_owned())?;
        let gate = pipeline.by_name("gate").ok_or("no valve")?;
        let sink = pipeline
            .by_name("sink")
            .ok_or("no sink")?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| "not an appsink".to_owned())?;
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| e.to_string())?;
        let (requests, wanted) = std_mpsc::channel();
        let (frames_tx, frames) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("screen-pattern".into())
            .spawn(move || serve(&pipeline, &gate, &sink, &wanted, &frames_tx))
            .map_err(|e| e.to_string())?;
        tracing::info!(?format, "second screen: test pattern started");
        Ok(Self {
            requests,
            frames,
            asked: false,
        })
    }
}

/// The pipeline's thread: one encoded frame per request, until the source
/// is dropped (its sender closes) or the pipeline fails.
fn serve(
    pipeline: &gst::Pipeline,
    gate: &gst::Element,
    sink: &gst_app::AppSink,
    wanted: &std_mpsc::Receiver<Request>,
    frames: &mpsc::UnboundedSender<Option<EncodedFrame>>,
) {
    while let Ok(request) = wanted.recv() {
        match request {
            Request::KeyFrame => {
                let event = gst::event::CustomUpstream::new(
                    gst::Structure::builder("GstForceKeyUnit")
                        .field("all-headers", true)
                        .build(),
                );
                sink.send_event(event);
            }
            Request::Frame => {
                gate.set_property("drop", false);
                let sample = sink.pull_sample();
                gate.set_property("drop", true);
                let frame = sample.ok().and_then(|s| encoded(&s));
                let ended = frame.is_none();
                if frames.send(frame).is_err() || ended {
                    break;
                }
            }
        }
    }
    let _ = pipeline.set_state(gst::State::Null);
    tracing::info!("second screen: test pattern stopped");
}

fn encoded(sample: &gst::Sample) -> Option<EncodedFrame> {
    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    Some(EncodedFrame {
        key_frame: !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT),
        // Running time of the picture, as the overlay shows it.
        capture_us: buffer.pts().map_or(0, gst::ClockTime::useconds),
        data: map.as_slice().to_vec(),
    })
}

impl FrameSource for PatternSource {
    fn next_frame(&mut self) -> Pin<Box<dyn Future<Output = Option<EncodedFrame>> + Send + '_>> {
        Box::pin(async move {
            // One request at a time: a cancelled wait keeps its request,
            // whose frame is taken by the next call.
            if !self.asked {
                self.requests.send(Request::Frame).ok()?;
                self.asked = true;
            }
            let frame = self.frames.recv().await.flatten();
            self.asked = false;
            frame
        })
    }

    fn request_key_frame(&mut self) {
        let _ = self.requests.send(Request::KeyFrame);
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
            assert!(p.contains("valve name=gate drop=true"), "{p}");
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
        // Nothing comes without a request.
        assert!(source.frames.try_recv().is_err());
        source.request_key_frame();
        let mut key = false;
        for _ in 0..5 {
            key |= source.next_frame().await.unwrap().key_frame;
        }
        assert!(key, "no key frame after the request");
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
