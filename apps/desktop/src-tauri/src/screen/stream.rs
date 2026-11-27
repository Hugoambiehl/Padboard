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

/// Serves `job` on `ws` until it ends.
pub async fn serve<S>(ws: &mut WebSocketStream<S>, mut job: Job) -> StreamEnd
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut flow = Flow::default();
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
            }
            () = tokio::time::sleep_until(stall_deadline), if flow.in_flight() > 0 => {
                let _ = ws.close(None).await;
                return StreamEnd::Stalled;
            }
        }
    }
}
