//! The loop of a stream connection: frames out while the flow allows,
//! acknowledgements and key frame requests in.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use padboard_protocol::StreamFrame;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::Job;
use super::flow::Flow;

/// No acknowledgement for this long with frames in flight: the tablet is
/// gone or stuck, the stream ends.
pub const STALL: Duration = Duration::from_secs(10);

/// Why a stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// Stopped on purpose (PC, tablet, or replaced).
    Stopped,
    /// The tablet closed the connection, or it broke.
    Closed,
    /// The source ended (capture lost, encoder failed).
    SourceEnded,
    /// No acknowledgement for [`STALL`].
    Stalled,
}

/// Frames between two lines of statistics in the log.
const REPORT_EVERY: usize = 300;

/// Send → ack times of the last frames, for the log: what the tablet adds
/// to the encoding (transfer, decode, display).
#[derive(Debug, Default)]
struct Stats {
    sent: std::collections::HashMap<u64, Instant>,
    rtt_ms: Vec<f64>,
    since: Option<Instant>,
}

impl Stats {
    fn sent(&mut self, capture_us: u64) {
        self.since.get_or_insert_with(Instant::now);
        self.sent.insert(capture_us, Instant::now());
    }

    fn acked(&mut self, capture_us: u64) {
        let Some(at) = self.sent.remove(&capture_us) else {
            return;
        };
        // Frames skipped by the decoder are never acked: forget older ones.
        self.sent.retain(|&t, _| t > capture_us);
        self.rtt_ms.push(at.elapsed().as_secs_f64() * 1000.0);
        if self.rtt_ms.len() < REPORT_EVERY {
            return;
        }
        let seconds = self.since.map_or(1.0, |s| s.elapsed().as_secs_f64());
        let mut rtt = std::mem::take(&mut self.rtt_ms);
        rtt.sort_by(f64::total_cmp);
        let pick = |q: f64| rtt[((rtt.len() - 1) as f64 * q) as usize];
        tracing::info!(
            fps = format!("{:.1}", REPORT_EVERY as f64 / seconds),
            ack_p50_ms = format!("{:.1}", pick(0.5)),
            ack_p90_ms = format!("{:.1}", pick(0.9)),
            "second screen stream"
        );
        self.since = Some(Instant::now());
    }
}

/// Serves `job` on `ws` until it ends.
pub async fn serve<S>(ws: &mut WebSocketStream<S>, mut job: Job) -> StreamEnd
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut flow = Flow::default();
    let mut stats = Stats::default();
    let mut last_progress = Instant::now();
    loop {
        if *job.stop.borrow() {
            let _ = ws.close(None).await;
            return StreamEnd::Stopped;
        }
        let stall_deadline = last_progress + STALL;
        tokio::select! {
            changed = job.stop.changed() => {
                if changed.is_err() || *job.stop.borrow() {
                    let _ = ws.close(None).await;
                    return StreamEnd::Stopped;
                }
            }
            message = ws.next() => match message {
                Some(Ok(WsMessage::Binary(bytes))) => match StreamFrame::decode(&bytes) {
                    Ok(StreamFrame::Ack { capture_us, .. }) => {
                        flow.acked(capture_us);
                        stats.acked(capture_us);
                        last_progress = Instant::now();
                    }
                    Ok(StreamFrame::KeyFrameRequest) => job.source.request_key_frame(),
                    // A video frame from the tablet, or a kind from a newer
                    // tablet: ignored.
                    Ok(StreamFrame::Video { .. }) | Err(_) => {}
                },
                Some(Ok(WsMessage::Close(_)) | Err(_)) | None => return StreamEnd::Closed,
                Some(Ok(_)) => {}
            },
            frame = job.source.next_frame(), if flow.can_send() => {
                let Some(frame) = frame else {
                    let _ = ws.close(None).await;
                    return StreamEnd::SourceEnded;
                };
                let bytes = StreamFrame::Video {
                    key_frame: frame.key_frame,
                    capture_us: frame.capture_us,
                    data: &frame.data,
                }
                .encode();
                if ws.send(WsMessage::binary(bytes)).await.is_err() {
                    return StreamEnd::Closed;
                }
                if flow.in_flight() == 0 {
                    last_progress = Instant::now();
                }
                flow.sent(frame.capture_us);
                stats.sent(frame.capture_us);
            }
            () = tokio::time::sleep_until(stall_deadline), if flow.in_flight() > 0 => {
                let _ = ws.close(None).await;
                return StreamEnd::Stalled;
            }
        }
    }
}
