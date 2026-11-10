//! WebSocket server the tablets connect to.
//!
//! Independent from Tauri: it reports what happens through a channel of
//! [`ServerEvent`]s, which the app turns into UI state (and tests inspect).
//!
//! Per connection:
//! 1. the tablet must send `hello` within [`HANDSHAKE_TIMEOUT`] with a
//!    compatible protocol version, otherwise it gets an `error` and is closed;
//! 2. the server answers `welcome` then `layout:sync`;
//! 3. `button:press` runs the bound action, `ping` gets a `pong`.
//!
//! Phase 1 has no authentication (see `settings.rs` for the bind policy).
//! Handshakes carrying an `Origin` header are rejected: browsers always send
//! one, the tablet never does, so a web page open on the PC cannot drive the
//! server (cross-site WebSocket hijacking).

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use padboard_actions::{Action, ActionError, ActionExecutor};
use padboard_protocol::{
    ButtonEvent, ErrorCode, ErrorPayload, Heartbeat, HelloPayload, Message, PROTOCOL_VERSION,
    WelcomePayload, is_compatible,
};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self, Message as WsMessage};

use crate::demo::Profile;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Messages from the tablet are tiny; this bounds memory per connection.
const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// Everything a connection handler needs.
#[derive(Debug)]
pub struct ServerContext {
    pub server_name: String,
    pub profile: Profile,
    pub executor: Arc<dyn ActionExecutor>,
    pub events: UnboundedSender<ServerEvent>,
}

/// What happened, for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    Listening(SocketAddr),
    BindFailed {
        addr: SocketAddr,
        error: String,
    },
    ClientConnected {
        id: u64,
        device_name: String,
        peer: SocketAddr,
    },
    ClientDisconnected {
        id: u64,
    },
    ButtonPressed {
        client_id: u64,
        button_id: String,
        outcome: ActionOutcome,
    },
}

/// Result of a button press, as shown in the activity log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ActionOutcome {
    Ok,
    /// `code` is a stable identifier the UI translates; `detail` is the
    /// English developer message.
    Error {
        code: String,
        detail: String,
    },
}

impl ActionOutcome {
    fn from_error(e: &ActionError) -> Self {
        let code = match e {
            ActionError::Invalid(_) => "invalid",
            ActionError::Unsupported { .. } => "unsupported",
            ActionError::Unavailable(_) => "unavailable",
            _ => "failed",
        };
        Self::Error {
            code: code.to_owned(),
            detail: e.to_string(),
        }
    }
}

/// Binds `addr` and serves forever. Reports [`ServerEvent::BindFailed`] and
/// returns if the address cannot be bound.
pub async fn run(addr: SocketAddr, ctx: Arc<ServerContext>) {
    match TcpListener::bind(addr).await {
        Ok(listener) => serve(listener, ctx).await,
        Err(e) => {
            tracing::error!(%addr, "cannot listen: {e}");
            emit(
                &ctx,
                ServerEvent::BindFailed {
                    addr,
                    error: e.to_string(),
                },
            );
        }
    }
}

/// Serves connections accepted on an already bound listener.
pub async fn serve(listener: TcpListener, ctx: Arc<ServerContext>) {
    let next_id = AtomicU64::new(1);
    match listener.local_addr() {
        Ok(addr) => {
            tracing::info!(%addr, "WebSocket server listening");
            emit(&ctx, ServerEvent::Listening(addr));
        }
        Err(e) => tracing::warn!("cannot read the listening address: {e}"),
    }
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let id = next_id.fetch_add(1, Ordering::Relaxed);
                let ctx = Arc::clone(&ctx);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer, id, &ctx).await {
                        tracing::info!(%peer, client = id, "connection ended: {e}");
                    }
                });
            }
            Err(e) => {
                // Typically EMFILE; back off instead of spinning.
                tracing::warn!("accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

fn emit(ctx: &ServerContext, event: ServerEvent) {
    // The receiver only disappears when the app shuts down.
    let _ = ctx.events.send(event);
}

#[derive(Debug, thiserror::Error)]
enum ConnectionError {
    #[error("websocket error: {0}")]
    Ws(#[from] tungstenite::Error),
    #[error("no hello within {HANDSHAKE_TIMEOUT:?}")]
    HandshakeTimeout,
    #[error("handshake rejected: {0}")]
    HandshakeRejected(String),
    #[error("cannot serialize a message: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("closed by peer")]
    Closed,
}

type Ws = WebSocketStream<TcpStream>;

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    id: u64,
    ctx: &ServerContext,
) -> Result<(), ConnectionError> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_SIZE))
        .max_frame_size(Some(MAX_MESSAGE_SIZE));
    let mut ws =
        tokio_tungstenite::accept_hdr_async_with_config(stream, reject_browsers, Some(config))
            .await?;

    let hello = match timeout(HANDSHAKE_TIMEOUT, next_message(&mut ws)).await {
        Err(_) => return Err(ConnectionError::HandshakeTimeout),
        Ok(Err(e)) => return Err(e),
        Ok(Ok(Ok(Message::Hello(hello)))) => hello,
        Ok(Ok(Ok(other))) => {
            let reason = format!("expected hello, got {}", other.type_tag());
            return reject(&mut ws, ErrorCode::BadRequest, reason).await;
        }
        Ok(Ok(Err(reason))) => return reject(&mut ws, ErrorCode::BadRequest, reason).await,
    };
    let HelloPayload {
        protocol_version,
        device_name,
    } = hello;
    if !is_compatible(protocol_version) {
        let reason = format!(
            "protocol version {protocol_version} is not supported (expected {PROTOCOL_VERSION})"
        );
        return reject(&mut ws, ErrorCode::UnsupportedVersion, reason).await;
    }

    send(
        &mut ws,
        &Message::Welcome(WelcomePayload {
            protocol_version: PROTOCOL_VERSION,
            server_name: ctx.server_name.clone(),
        }),
    )
    .await?;
    send(&mut ws, &Message::LayoutSync(ctx.profile.layout.clone())).await?;

    tracing::info!(%peer, client = id, device = %device_name, "tablet connected");
    emit(
        ctx,
        ServerEvent::ClientConnected {
            id,
            device_name,
            peer,
        },
    );
    let result = session(&mut ws, id, ctx).await;
    // Completes the closing handshake (replies to the peer's close frame, or
    // sends ours) so the tablet sees a clean close instead of a dropped socket.
    // `SinkExt::close`, not the inherent `WebSocketStream::close`, which would try
    // to send a new close frame instead of finishing the handshake.
    let _ = SinkExt::close(&mut ws).await;
    emit(ctx, ServerEvent::ClientDisconnected { id });
    tracing::info!(%peer, client = id, "tablet disconnected");
    match result {
        Err(ConnectionError::Closed) => Ok(()),
        other => other,
    }
}

/// Main loop once the handshake is done.
async fn session(ws: &mut Ws, id: u64, ctx: &ServerContext) -> Result<(), ConnectionError> {
    loop {
        let message = match next_message(ws).await? {
            Ok(m) => m,
            Err(reason) => {
                send_error(ws, ErrorCode::BadRequest, reason).await?;
                continue;
            }
        };
        match message {
            Message::ButtonPress(ButtonEvent { button_id }) => {
                press(ws, id, ctx, button_id).await?;
            }
            // Long presses arrive in phase 4.
            Message::ButtonRelease(_) | Message::Pong(_) => {}
            Message::Ping(Heartbeat { seq }) => {
                send(ws, &Message::Pong(Heartbeat { seq })).await?;
            }
            other @ (Message::Hello(_)
            | Message::Welcome(_)
            | Message::Error(_)
            | Message::LayoutSync(_)) => {
                let reason = format!("unexpected {} after handshake", other.type_tag());
                send_error(ws, ErrorCode::BadRequest, reason).await?;
            }
        }
    }
}

async fn press(
    ws: &mut Ws,
    client_id: u64,
    ctx: &ServerContext,
    button_id: String,
) -> Result<(), ConnectionError> {
    let Some(action) = ctx.profile.actions.get(&button_id).cloned() else {
        let reason = format!("no button {button_id:?} in the current layout");
        return send_error(ws, ErrorCode::UnknownButton, reason).await;
    };
    let outcome = execute(ctx, action).await;
    if let ActionOutcome::Error { detail, .. } = &outcome {
        send_error(ws, ErrorCode::ActionFailed, detail.clone()).await?;
    }
    emit(
        ctx,
        ServerEvent::ButtonPressed {
            client_id,
            button_id,
            outcome,
        },
    );
    Ok(())
}

/// Runs the action on a blocking thread (executors are blocking).
async fn execute(ctx: &ServerContext, action: Action) -> ActionOutcome {
    let executor = Arc::clone(&ctx.executor);
    match tokio::task::spawn_blocking(move || executor.execute(&action)).await {
        Ok(Ok(())) => ActionOutcome::Ok,
        Ok(Err(e)) => ActionOutcome::from_error(&e),
        Err(join_error) => ActionOutcome::Error {
            code: "failed".to_owned(),
            detail: format!("action task crashed: {join_error}"),
        },
    }
}

/// Next protocol message. The outer error ends the connection; the inner
/// `Err(reason)` is a malformed message the session may survive.
async fn next_message(ws: &mut Ws) -> Result<Result<Message, String>, ConnectionError> {
    loop {
        match ws.next().await {
            None | Some(Ok(WsMessage::Close(_))) => return Err(ConnectionError::Closed),
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(WsMessage::Text(text))) => {
                return Ok(Message::from_json(&text).map_err(|e| format!("invalid message: {e}")));
            }
            Some(Ok(WsMessage::Binary(_))) => {
                return Ok(Err("binary frames are not supported".to_owned()));
            }
            // Control frames: tungstenite answers pings by itself.
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_))) => {}
        }
    }
}

async fn send(ws: &mut Ws, message: &Message) -> Result<(), ConnectionError> {
    ws.send(WsMessage::text(message.to_json()?)).await?;
    Ok(())
}

async fn send_error(ws: &mut Ws, code: ErrorCode, message: String) -> Result<(), ConnectionError> {
    tracing::debug!(?code, "{message}");
    send(ws, &Message::Error(ErrorPayload { code, message })).await
}

/// Sends a fatal error, closes the socket and ends the connection.
async fn reject(ws: &mut Ws, code: ErrorCode, reason: String) -> Result<(), ConnectionError> {
    send_error(ws, code, reason.clone()).await?;
    let _ = SinkExt::close(ws).await;
    Err(ConnectionError::HandshakeRejected(reason))
}

/// Handshake callback: refuses requests coming from a browser.
#[allow(clippy::result_large_err)] // signature imposed by tungstenite
fn reject_browsers(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    if request.headers().contains_key("origin") {
        tracing::warn!("rejected a WebSocket handshake with an Origin header (browser)");
        let mut error = ErrorResponse::new(Some("browsers are not allowed".to_owned()));
        *error.status_mut() = StatusCode::FORBIDDEN;
        return Err(error);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use padboard_actions::{Hotkey, Key};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{MaybeTlsStream, connect_async};

    use super::*;

    type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

    #[derive(Debug, Default)]
    struct FakeExecutor {
        executed: Mutex<Vec<Action>>,
        fail: bool,
    }

    impl ActionExecutor for FakeExecutor {
        fn execute(&self, action: &Action) -> Result<(), ActionError> {
            self.executed.lock().unwrap().push(action.clone());
            if self.fail {
                Err(ActionError::Unavailable("no uinput".into()))
            } else {
                Ok(())
            }
        }
    }

    struct Harness {
        url: String,
        executor: Arc<FakeExecutor>,
        events: UnboundedReceiver<ServerEvent>,
    }

    async fn start(fail: bool) -> Harness {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let executor = Arc::new(FakeExecutor {
            fail,
            ..Default::default()
        });
        let (tx, events) = unbounded_channel();
        let ctx = Arc::new(ServerContext {
            server_name: "TEST-PC".into(),
            profile: crate::demo::profile(),
            executor: Arc::clone(&executor) as Arc<dyn ActionExecutor>,
            events: tx,
        });
        tokio::spawn(serve(listener, ctx));
        Harness {
            url,
            executor,
            events,
        }
    }

    async fn send_msg(client: &mut Client, m: &Message) {
        client
            .send(WsMessage::text(m.to_json().unwrap()))
            .await
            .unwrap();
    }

    async fn recv_msg(client: &mut Client) -> Message {
        let frame = timeout(Duration::from_secs(5), client.next())
            .await
            .expect("timed out waiting for a message")
            .unwrap()
            .unwrap();
        Message::from_json(frame.to_text().unwrap()).unwrap()
    }

    fn hello(version: u32) -> Message {
        Message::Hello(HelloPayload {
            protocol_version: version,
            device_name: "Test tablet".into(),
        })
    }

    async fn connected(h: &Harness) -> Client {
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, &hello(PROTOCOL_VERSION)).await;
        let Message::Welcome(w) = recv_msg(&mut client).await else {
            panic!("expected welcome");
        };
        assert_eq!(w.server_name, "TEST-PC");
        assert!(matches!(
            recv_msg(&mut client).await,
            Message::LayoutSync(_)
        ));
        client
    }

    async fn next_event(h: &mut Harness) -> ServerEvent {
        timeout(Duration::from_secs(5), h.events.recv())
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn handshake_sends_welcome_then_layout() {
        let mut h = start(false).await;
        assert!(matches!(
            next_event(&mut h).await,
            ServerEvent::Listening(_)
        ));
        let _client = connected(&h).await;
        let ServerEvent::ClientConnected { device_name, .. } = next_event(&mut h).await else {
            panic!("expected ClientConnected");
        };
        assert_eq!(device_name, "Test tablet");
    }

    #[tokio::test]
    async fn incompatible_version_is_rejected() {
        let h = start(false).await;
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, &hello(PROTOCOL_VERSION + 1)).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::UnsupportedVersion);
        // The server then closes the connection.
        let next = timeout(Duration::from_secs(5), client.next())
            .await
            .unwrap();
        assert!(matches!(
            next,
            None | Some(Ok(WsMessage::Close(_))) | Some(Err(_))
        ));
    }

    #[tokio::test]
    async fn first_message_must_be_hello() {
        let h = start(false).await;
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 1 })).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::BadRequest);
    }

    #[tokio::test]
    async fn press_runs_the_bound_action() {
        let mut h = start(false).await;
        let mut client = connected(&h).await;
        send_msg(
            &mut client,
            &Message::ButtonPress(ButtonEvent {
                button_id: "terminal".into(),
            }),
        )
        .await;
        loop {
            if let ServerEvent::ButtonPressed {
                button_id, outcome, ..
            } = next_event(&mut h).await
            {
                assert_eq!(button_id, "terminal");
                assert_eq!(outcome, ActionOutcome::Ok);
                break;
            }
        }
        assert_eq!(
            *h.executor.executed.lock().unwrap(),
            [Action::Hotkey(Hotkey::new([
                Key::ControlLeft,
                Key::AltLeft,
                Key::KeyT
            ]))]
        );
    }

    #[tokio::test]
    async fn failed_action_is_reported_to_the_tablet() {
        let h = start(true).await;
        let mut client = connected(&h).await;
        send_msg(
            &mut client,
            &Message::ButtonPress(ButtonEvent {
                button_id: "copy".into(),
            }),
        )
        .await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::ActionFailed);
    }

    #[tokio::test]
    async fn unknown_button_is_reported() {
        let h = start(false).await;
        let mut client = connected(&h).await;
        send_msg(
            &mut client,
            &Message::ButtonPress(ButtonEvent {
                button_id: "nope".into(),
            }),
        )
        .await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::UnknownButton);
        assert!(h.executor.executed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ping_gets_pong_and_garbage_is_survived() {
        let h = start(false).await;
        let mut client = connected(&h).await;
        client.send(WsMessage::text("{not json")).await.unwrap();
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::BadRequest);
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 7 })).await;
        assert_eq!(
            recv_msg(&mut client).await,
            Message::Pong(Heartbeat { seq: 7 })
        );
    }

    #[tokio::test]
    async fn client_close_gets_a_clean_close_reply() {
        let h = start(false).await;
        let mut client = connected(&h).await;
        client.close(None).await.unwrap();
        let reply = timeout(Duration::from_secs(5), client.next())
            .await
            .expect("no close reply");
        assert!(
            matches!(reply, Some(Ok(WsMessage::Close(_))) | None),
            "unexpected {reply:?}"
        );
    }

    #[tokio::test]
    async fn browser_handshake_is_rejected() {
        let h = start(false).await;
        let mut request = h.url.as_str().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("origin", "https://evil.example".parse().unwrap());
        let err = connect_async(request).await.unwrap_err();
        let tungstenite::Error::Http(response) = err else {
            panic!("expected an HTTP error, got {err:?}");
        };
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn bind_failure_is_reported() {
        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = taken.local_addr().unwrap();
        let (tx, mut rx) = unbounded_channel();
        let ctx = Arc::new(ServerContext {
            server_name: "TEST-PC".into(),
            profile: crate::demo::profile(),
            executor: Arc::new(FakeExecutor::default()),
            events: tx,
        });
        run(addr, ctx).await;
        assert!(matches!(
            rx.recv().await,
            Some(ServerEvent::BindFailed { addr: a, .. }) if a == addr
        ));
    }
}
