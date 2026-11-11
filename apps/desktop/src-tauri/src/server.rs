//! WebSocket server the tablets connect to.
//!
//! Independent from Tauri: it reports what happens through a channel of
//! [`ServerEvent`]s, which the app turns into UI state (and tests inspect).
//!
//! Two transports share this code (see `auth.rs`):
//! - **USB**: loopback port reached through `adb reverse`, trusted;
//! - **Wi-Fi**: LAN port, a token (or a PIN pairing) is required.
//!
//! Per connection:
//! 1. the tablet must send `hello` within [`Limits::handshake`] with a
//!    compatible protocol version, otherwise it gets an `error` and is closed;
//! 2. the server answers `welcome` with the auth status. Until authenticated,
//!    only `pair:request` is accepted (anything else: `unauthorized`, closed),
//!    for at most [`Limits::unauthenticated`]; at most [`Limits::max_pairing`]
//!    such connections at once. Wi-Fi connections that have not sent `hello`
//!    yet are capped separately ([`Limits::max_handshaking`]), so pairing
//!    attempts can never lock a paired tablet out;
//! 3. once authenticated: `layout:sync`, then `button:press` runs the bound
//!    action and `ping` gets a `pong`. A newer session of the same device or a
//!    revocation ends the session (`superseded` / `revoked`).
//!
//! Handshakes carrying an `Origin` header are rejected: browsers always send
//! one, the tablet never does, so a web page open on the PC cannot drive the
//! server (cross-site WebSocket hijacking).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use padboard_actions::{Action, ActionError, ActionExecutor, NavigateAction, NavigateTo};
use padboard_protocol::{
    AuthStatus, ButtonEvent, ErrorCode, ErrorPayload, Heartbeat, HelloPayload, Layout, Message,
    NavigatePayload, NavigateTarget, PROTOCOL_VERSION, PairSuccessPayload, WelcomePayload,
    is_compatible,
};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self, Message as WsMessage};

use crate::appearance::{self, SystemState};
use crate::auth::{Auth, AuthChange, Transport};
use crate::config::ActiveLayout;

/// Messages from the tablet are tiny; this bounds memory per connection.
const MAX_MESSAGE_SIZE: usize = 64 * 1024;
/// Longest device id / name accepted in `hello` (characters).
const MAX_ID_LEN: usize = 64;
const MAX_NAME_LEN: usize = 64;

/// Timeouts and limits (shortened in tests).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub handshake: Duration,
    pub unauthenticated: Duration,
    /// Wi-Fi connections waiting for their `hello`.
    pub max_handshaking: usize,
    /// Connections in the pairing phase (no valid token).
    pub max_pairing: usize,
    /// An authenticated session silent for this long is closed (the tablet
    /// pings every 5 s; a Wi-Fi drop leaves no TCP close behind).
    pub idle: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(5),
            unauthenticated: crate::auth::PIN_LIFETIME,
            max_handshaking: 32,
            max_pairing: 4,
            idle: Duration::from_secs(20),
        }
    }
}

/// Everything a connection handler needs.
#[derive(Debug)]
pub struct ServerContext {
    pub server_name: String,
    pub server_id: String,
    /// The active profile's layout and actions; sessions push every new
    /// value to their tablet (`layout:sync`).
    pub layout: watch::Sender<Arc<ActiveLayout>>,
    /// What the PC observes for button states (`appearance.rs`); sessions
    /// send `button:update` for the buttons it changes.
    pub state: watch::Sender<SystemState>,
    /// Set once created (creating it may take a moment, see `lib.rs`).
    pub executor: OnceLock<Arc<dyn ActionExecutor>>,
    pub auth: Arc<Auth>,
    pub events: UnboundedSender<ServerEvent>,
    pub limits: Limits,
    /// Connection ids, unique across both transports.
    pub next_connection_id: AtomicU64,
    /// Wi-Fi connections waiting for their `hello`.
    pub handshaking: AtomicUsize,
    /// Connections in the pairing phase.
    pub pairing: AtomicUsize,
    /// Multi actions running, by button: a second press while one runs is
    /// ignored (two sequences of keys would interleave).
    pub running: std::sync::Mutex<std::collections::HashSet<String>>,
}

/// What happened, for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    Listening {
        transport: Transport,
        addr: SocketAddr,
    },
    BindFailed {
        transport: Transport,
        addr: SocketAddr,
        error: String,
    },
    /// The listener was stopped (Wi-Fi turned off).
    Stopped {
        transport: Transport,
    },
    /// Wi-Fi turned on or off from the UI.
    WifiEnabled(bool),
    Mdns(crate::wifi::MdnsStatus),
    /// The active layout changed (edited, or another profile activated).
    LayoutChanged(Arc<ActiveLayout>),
    /// IPv4 addresses of the PC changed (physical interfaces first).
    LanAddresses(Vec<crate::netif::LanAddress>),
    /// `adb` and the USB devices it sees.
    Usb(crate::adb::UsbStatus),
    ClientConnected {
        id: u64,
        device_id: String,
        device_name: String,
        peer: SocketAddr,
        transport: Transport,
    },
    ClientDisconnected {
        id: u64,
    },
    ButtonPressed {
        client_id: u64,
        button_id: String,
        outcome: ActionOutcome,
    },
    Auth(AuthChange),
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
/// returns if the address cannot be bound. `tls`: wrap connections in TLS
/// (the Wi-Fi port).
pub async fn run(
    addr: SocketAddr,
    transport: Transport,
    tls: Option<TlsAcceptor>,
    ctx: Arc<ServerContext>,
) {
    match TcpListener::bind(addr).await {
        Ok(listener) => serve(listener, transport, tls, ctx).await,
        Err(e) => {
            tracing::error!(%addr, ?transport, "cannot listen: {e}");
            emit(
                &ctx,
                ServerEvent::BindFailed {
                    transport,
                    addr,
                    error: e.to_string(),
                },
            );
        }
    }
}

/// Serves connections accepted on an already bound listener. Connection tasks
/// are owned by this future: dropping it (e.g. aborting its task) closes them.
pub async fn serve(
    listener: TcpListener,
    transport: Transport,
    tls: Option<TlsAcceptor>,
    ctx: Arc<ServerContext>,
) {
    match listener.local_addr() {
        Ok(addr) => {
            tracing::info!(%addr, ?transport, "WebSocket server listening");
            emit(&ctx, ServerEvent::Listening { transport, addr });
        }
        Err(e) => tracing::warn!("cannot read the listening address: {e}"),
    }
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let guard = match transport {
                        Transport::Usb => None,
                        Transport::Wifi => {
                            let max = ctx.limits.max_handshaking;
                            match SlotGuard::acquire(&ctx, |c| &c.handshaking, max) {
                                Some(guard) => Some(guard),
                                None => {
                                    tracing::warn!(%peer, "too many pending connections, dropped");
                                    continue;
                                }
                            }
                        }
                    };
                    let id = ctx.next_connection_id.fetch_add(1, Ordering::Relaxed);
                    let ctx = Arc::clone(&ctx);
                    let tls = tls.clone();
                    connections.spawn(async move {
                        let result = match tls {
                            None => handle_connection(stream, peer, id, transport, guard, &ctx).await,
                            Some(tls) => match timeout(ctx.limits.handshake, tls.accept(stream)).await {
                                Ok(Ok(stream)) => {
                                    handle_connection(stream, peer, id, transport, guard, &ctx).await
                                }
                                Ok(Err(e)) => Err(ConnectionError::Tls(e)),
                                Err(_) => Err(ConnectionError::HandshakeTimeout),
                            },
                        };
                        if let Err(e) = result {
                            tracing::info!(%peer, client = id, "connection ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    // Typically EMFILE; back off instead of spinning.
                    tracing::warn!("accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            },
            // Reap finished connection tasks so the set does not grow.
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

/// Holds one slot of a bounded counter of the context until dropped.
struct SlotGuard {
    ctx: Arc<ServerContext>,
    counter: fn(&ServerContext) -> &AtomicUsize,
}

impl SlotGuard {
    fn acquire(
        ctx: &Arc<ServerContext>,
        counter: fn(&ServerContext) -> &AtomicUsize,
        max: usize,
    ) -> Option<Self> {
        let previous = counter(ctx).fetch_add(1, Ordering::AcqRel);
        if previous >= max {
            counter(ctx).fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Self {
            ctx: Arc::clone(ctx),
            counter,
        })
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        (self.counter)(&self.ctx).fetch_sub(1, Ordering::AcqRel);
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
    #[error("TLS handshake failed: {0}")]
    Tls(std::io::Error),
    #[error("no hello in time")]
    HandshakeTimeout,
    #[error("rejected: {0}")]
    Rejected(String),
    #[error("cannot serialize a message: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("closed by peer")]
    Closed,
    #[error("session ended by the server: {0:?}")]
    Kicked(crate::auth::Kick),
    #[error("no message for too long")]
    Idle,
}

async fn handle_connection<S>(
    stream: S,
    peer: SocketAddr,
    id: u64,
    transport: Transport,
    handshaking: Option<SlotGuard>,
    ctx: &Arc<ServerContext>,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_SIZE))
        .max_frame_size(Some(MAX_MESSAGE_SIZE));
    // The upgrade is bounded too: a silent TCP connection must not hold a slot.
    let upgrade =
        tokio_tungstenite::accept_hdr_async_with_config(stream, reject_browsers, Some(config));
    let mut ws = match timeout(ctx.limits.handshake, upgrade).await {
        Ok(ws) => ws?,
        Err(_) => return Err(ConnectionError::HandshakeTimeout),
    };

    let hello = match timeout(ctx.limits.handshake, next_message(&mut ws)).await {
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
        device_id,
        token,
    } = hello;
    if !is_compatible(protocol_version) {
        let reason = format!(
            "protocol version {protocol_version} is not supported (expected {PROTOCOL_VERSION})"
        );
        return reject(&mut ws, ErrorCode::UnsupportedVersion, reason).await;
    }
    if device_id.is_empty()
        || device_id.chars().count() > MAX_ID_LEN
        || device_id.chars().any(char::is_control)
    {
        return reject(&mut ws, ErrorCode::BadRequest, "invalid deviceId".into()).await;
    }
    let device_name = clean_name(&device_name);
    drop(handshaking);

    let auth = match transport {
        Transport::Usb => AuthStatus::Authenticated,
        Transport::Wifi => ctx
            .auth
            .authenticate(&device_id, &device_name, token.as_deref()),
    };
    if auth == AuthStatus::TokenRejected {
        tracing::warn!(%peer, device = %device_name, "unknown or revoked token");
    }
    let pairing_slot = if auth == AuthStatus::Authenticated {
        None
    } else {
        match SlotGuard::acquire(ctx, |c| &c.pairing, ctx.limits.max_pairing) {
            Some(slot) => Some(slot),
            None => {
                let reason = "too many pairing attempts in progress".to_owned();
                return reject(&mut ws, ErrorCode::Unauthorized, reason).await;
            }
        }
    };
    send(
        &mut ws,
        &Message::Welcome(WelcomePayload {
            protocol_version: PROTOCOL_VERSION,
            server_name: ctx.server_name.clone(),
            server_id: ctx.server_id.clone(),
            auth,
        }),
    )
    .await?;

    if auth != AuthStatus::Authenticated {
        match timeout(
            ctx.limits.unauthenticated,
            pairing(&mut ws, &device_id, &device_name, ctx),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                let reason = "no pairing in time".to_owned();
                return reject(&mut ws, ErrorCode::Unauthorized, reason).await;
            }
        }
    }
    drop(pairing_slot);

    // Subscribe before reading, so an edit or a state change landing in
    // between is not missed. The layout carries every button's current state.
    let mut feeds = Feeds {
        layouts: ctx.layout.subscribe(),
        states: ctx.state.subscribe(),
        sent: None,
    };
    feeds.sync(&mut ws).await?;
    tracing::info!(%peer, client = id, ?transport, device = %device_name, "tablet connected");
    emit(
        ctx,
        ServerEvent::ClientConnected {
            id,
            device_id: device_id.clone(),
            device_name,
            peer,
            transport,
        },
    );
    let kick = ctx.auth.open_session(&device_id, id, transport);
    let result = session(&mut ws, id, kick, feeds, ctx).await;
    ctx.auth.close_session(&device_id, id);
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

/// Trims the device name and bounds its length (it is displayed in the UI).
fn clean_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_LEN)
        .collect();
    name.trim().to_owned()
}

/// Unauthenticated phase: only `pair:request` is accepted. Returns once paired.
async fn pairing<S>(
    ws: &mut WebSocketStream<S>,
    device_id: &str,
    device_name: &str,
    ctx: &ServerContext,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match next_message(ws).await? {
            Ok(Message::PairRequest(request)) => {
                match ctx.auth.pair(device_id, device_name, &request.pin) {
                    Ok(token) => {
                        send(ws, &Message::PairSuccess(PairSuccessPayload { token })).await?;
                        return Ok(());
                    }
                    Err(failure) => send(ws, &Message::PairFailure(failure)).await?,
                }
            }
            Ok(other) => {
                let reason = format!("{} requires an authenticated connection", other.type_tag());
                return reject(ws, ErrorCode::Unauthorized, reason).await;
            }
            Err(reason) => return reject(ws, ErrorCode::BadRequest, reason).await,
        }
    }
}

/// Main loop once authenticated.
async fn session<S>(
    ws: &mut WebSocketStream<S>,
    id: u64,
    mut kick: tokio::sync::oneshot::Receiver<crate::auth::Kick>,
    mut feeds: Feeds,
    ctx: &Arc<ServerContext>,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Messages sent on behalf of what presses started (navigation, the
    // outcome of a multi action running in its own task).
    let (outbox, mut outgoing) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let mut kick_pending = true;
    let mut layouts_open = true;
    let mut states_open = true;
    loop {
        let message = tokio::select! {
            message = timeout(ctx.limits.idle, next_message(ws)) => match message {
                Ok(message) => message?,
                Err(_) => return Err(ConnectionError::Idle),
            },
            changed = feeds.layouts.changed(), if layouts_open => {
                if changed.is_ok() {
                    feeds.sync(ws).await?;
                } else {
                    layouts_open = false; // the app is shutting down
                }
                continue;
            }
            Some(message) = outgoing.recv() => {
                send(ws, &message).await?;
                continue;
            }
            changed = feeds.states.changed(), if states_open => {
                if changed.is_ok() {
                    feeds.update(ws).await?;
                } else {
                    states_open = false;
                }
                continue;
            }
            kicked = &mut kick, if kick_pending => {
                kick_pending = false;
                // Err: the registry was dropped (shutdown); keep serving.
                if let Ok(reason) = kicked {
                    let detail = format!("session ended: {reason:?}");
                    send_error(ws, reason.error_code(), detail).await?;
                    return Err(ConnectionError::Kicked(reason));
                }
                continue;
            }
        };
        let message = match message {
            Ok(m) => m,
            Err(reason) => {
                send_error(ws, ErrorCode::BadRequest, reason).await?;
                continue;
            }
        };
        match message {
            Message::ButtonPress(ButtonEvent { button_id }) => {
                press(ws, id, ctx, &outbox, button_id).await?;
            }
            // Long presses arrive in phase 4.
            Message::ButtonRelease(_) | Message::Pong(_) => {}
            Message::Ping(Heartbeat { seq }) => {
                send(ws, &Message::Pong(Heartbeat { seq })).await?;
            }
            other @ (Message::Hello(_)
            | Message::Welcome(_)
            | Message::Error(_)
            | Message::LayoutSync(_)
            | Message::ButtonUpdate(_)
            | Message::Navigate(_)
            | Message::PairRequest(_)
            | Message::PairSuccess(_)
            | Message::PairFailure(_)) => {
                let reason = format!("unexpected {} after handshake", other.type_tag());
                send_error(ws, ErrorCode::BadRequest, reason).await?;
            }
        }
    }
}

/// What a session follows, and what its tablet shows.
struct Feeds {
    layouts: watch::Receiver<Arc<ActiveLayout>>,
    states: watch::Receiver<SystemState>,
    /// The resolved layout last sent.
    sent: Option<Layout>,
}

impl Feeds {
    fn resolved(&mut self) -> Layout {
        let active = Arc::clone(&self.layouts.borrow_and_update());
        let state = *self.states.borrow_and_update();
        appearance::resolve(&active.layout, &active.actions, &state)
    }

    /// New layout (edit, profile): sent whole, with the current states.
    async fn sync<S>(&mut self, ws: &mut WebSocketStream<S>) -> Result<(), ConnectionError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let layout = self.resolved();
        send(ws, &Message::LayoutSync(layout.clone())).await?;
        self.sent = Some(layout);
        Ok(())
    }

    /// New state: only the buttons whose look changed.
    async fn update<S>(&mut self, ws: &mut WebSocketStream<S>) -> Result<(), ConnectionError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let layout = self.resolved();
        let Some(sent) = &self.sent else {
            return self.sync(ws).await;
        };
        for button in appearance::changed(sent, &layout) {
            send(ws, &Message::ButtonUpdate(button)).await?;
        }
        self.sent = Some(layout);
        Ok(())
    }
}

/// Messages a session sends for the tasks its presses started.
type Outbox = tokio::sync::mpsc::UnboundedSender<Message>;

async fn press<S>(
    ws: &mut WebSocketStream<S>,
    client_id: u64,
    ctx: &Arc<ServerContext>,
    outbox: &Outbox,
    button_id: String,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let action = ctx.layout.borrow().actions.get(&button_id).cloned();
    let Some(action) = action else {
        let reason = format!("no button {button_id:?} in the current layout");
        return send_error(ws, ErrorCode::UnknownButton, reason).await;
    };
    if matches!(action, Action::Multi(_)) {
        // Steps and pauses can take seconds: in their own task, so the
        // session keeps answering pings and other buttons.
        let fresh = ctx
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(button_id.clone());
        if !fresh {
            tracing::debug!(button = %button_id, "multi action already running, press ignored");
            return Ok(());
        }
        let ctx = Arc::clone(ctx);
        let outbox = outbox.clone();
        tokio::spawn(async move {
            let outcome = perform(&ctx, action, Some(&outbox)).await;
            ctx.running
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&button_id);
            finish(&ctx, &outbox, client_id, button_id, outcome);
        });
        return Ok(());
    }
    let outcome = perform(ctx, action, Some(outbox)).await;
    finish(ctx, outbox, client_id, button_id, outcome);
    Ok(())
}

/// Reports the outcome of a press: an error to the tablet, the activity to the UI.
fn finish(
    ctx: &ServerContext,
    outbox: &Outbox,
    client_id: u64,
    button_id: String,
    outcome: ActionOutcome,
) {
    if let ActionOutcome::Error { detail, .. } = &outcome {
        let _ = outbox.send(Message::Error(ErrorPayload {
            code: ErrorCode::ActionFailed,
            message: detail.clone(),
        }));
    }
    emit(
        ctx,
        ServerEvent::ButtonPressed {
            client_id,
            button_id,
            outcome,
        },
    );
}

/// Carries out any action: system ones on the executor, navigation by a
/// message to the tablet that pressed (`outbox`; nothing without one, e.g.
/// "Tester l'action" in the editor), multi actions step by step. A failed
/// step stops the sequence.
pub async fn perform(
    ctx: &ServerContext,
    action: Action,
    outbox: Option<&Outbox>,
) -> ActionOutcome {
    let Action::Multi(multi) = action else {
        return perform_step(ctx, action, outbox).await;
    };
    let whole = Action::Multi(multi);
    if let Err(e) = whole.validate() {
        return ActionOutcome::from_error(&e);
    }
    let Action::Multi(multi) = whole else {
        unreachable!("just built")
    };
    for (i, step) in multi.steps.into_iter().enumerate() {
        if let ActionOutcome::Error { code, detail } = perform_step(ctx, step.action, outbox).await
        {
            return ActionOutcome::Error {
                code,
                detail: format!("step {}: {detail}", i + 1),
            };
        }
        if step.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(u64::from(step.delay_ms))).await;
        }
    }
    ActionOutcome::Ok
}

/// One action that is not a multi action (steps cannot be).
async fn perform_step(
    ctx: &ServerContext,
    action: Action,
    outbox: Option<&Outbox>,
) -> ActionOutcome {
    match action {
        Action::Navigate(nav) => {
            if let Err(e) = Action::Navigate(nav.clone()).validate() {
                return ActionOutcome::from_error(&e);
            }
            if let Some(outbox) = outbox {
                let _ = outbox.send(Message::Navigate(navigate_payload(&nav)));
            }
            ActionOutcome::Ok
        }
        Action::Multi(_) => ActionOutcome::from_error(&ActionError::Invalid(
            "a multi action cannot be a step".into(),
        )),
        Action::Obs(_) => ActionOutcome::from_error(&ActionError::Unsupported {
            action: "obs",
            platform: std::env::consts::OS,
        }),
        action => execute(ctx, action).await,
    }
}

fn navigate_payload(nav: &NavigateAction) -> NavigatePayload {
    NavigatePayload {
        to: match nav.to {
            NavigateTo::Page => NavigateTarget::Page,
            NavigateTo::Folder => NavigateTarget::Folder,
            NavigateTo::Next => NavigateTarget::Next,
            NavigateTo::Previous => NavigateTarget::Previous,
            NavigateTo::Back => NavigateTarget::Back,
        },
        id: nav.id.clone(),
    }
}

/// Runs the action on a blocking thread (executors are blocking).
pub async fn execute(ctx: &ServerContext, action: Action) -> ActionOutcome {
    let Some(executor) = ctx.executor.get().map(Arc::clone) else {
        return ActionOutcome::Error {
            code: "unavailable".to_owned(),
            detail: "the action executor is still starting".to_owned(),
        };
    };
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
async fn next_message<S>(
    ws: &mut WebSocketStream<S>,
) -> Result<Result<Message, String>, ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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

async fn send<S>(ws: &mut WebSocketStream<S>, message: &Message) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    ws.send(WsMessage::text(message.to_json()?)).await?;
    Ok(())
}

async fn send_error<S>(
    ws: &mut WebSocketStream<S>,
    code: ErrorCode,
    message: String,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tracing::debug!(?code, "{message}");
    send(ws, &Message::Error(ErrorPayload { code, message })).await
}

/// Sends a fatal error, closes the socket and ends the connection.
async fn reject<S>(
    ws: &mut WebSocketStream<S>,
    code: ErrorCode,
    reason: String,
) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    send_error(ws, code, reason.clone()).await?;
    let _ = SinkExt::close(ws).await;
    Err(ConnectionError::Rejected(reason))
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
    use padboard_protocol::{PairFailureReason, PairRequestPayload};
    use tokio::net::TcpStream;
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{MaybeTlsStream, connect_async};

    use super::*;
    use crate::auth::PairingView;

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
        auth: Arc<Auth>,
        layout: watch::Sender<Arc<ActiveLayout>>,
        state: watch::Sender<SystemState>,
        events: UnboundedReceiver<ServerEvent>,
    }

    fn context(
        executor: Arc<FakeExecutor>,
        events: UnboundedSender<ServerEvent>,
    ) -> Arc<ServerContext> {
        Arc::new(ServerContext {
            server_name: "TEST-PC".into(),
            server_id: "test-server".into(),
            layout: watch::channel(Arc::new(crate::config::seed::config().active_layout())).0,
            state: watch::channel(SystemState::default()).0,
            executor: OnceLock::from(executor as Arc<dyn ActionExecutor>),
            auth: Arc::new(Auth::new(None, Box::new(|_| {}))),
            events,
            limits: Limits {
                handshake: Duration::from_secs(5),
                unauthenticated: Duration::from_millis(500),
                max_handshaking: 3,
                max_pairing: 2,
                idle: Duration::from_secs(2),
            },
            next_connection_id: AtomicU64::new(1),
            handshaking: AtomicUsize::new(0),
            pairing: AtomicUsize::new(0),
            running: std::sync::Mutex::default(),
        })
    }

    async fn start(transport: Transport, fail: bool) -> Harness {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let executor = Arc::new(FakeExecutor {
            fail,
            ..Default::default()
        });
        let (tx, events) = unbounded_channel();
        let ctx = context(Arc::clone(&executor), tx);
        let auth = Arc::clone(&ctx.auth);
        let layout = ctx.layout.clone();
        let state = ctx.state.clone();
        tokio::spawn(serve(listener, transport, None, ctx));
        Harness {
            url,
            executor,
            auth,
            layout,
            state,
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

    /// Asserts the server closed the connection.
    async fn assert_closed(client: &mut Client) {
        let next = timeout(Duration::from_secs(5), client.next())
            .await
            .expect("the server did not close the connection");
        assert!(
            matches!(next, None | Some(Ok(WsMessage::Close(_)) | Err(_))),
            "unexpected {next:?}"
        );
    }

    fn hello_with(version: u32, device_id: &str, token: Option<&str>) -> Message {
        Message::Hello(HelloPayload {
            protocol_version: version,
            device_name: "Test tablet".into(),
            device_id: device_id.into(),
            token: token.map(Into::into),
        })
    }

    fn hello(version: u32) -> Message {
        hello_with(version, "test-device", None)
    }

    /// Sends hello and returns the welcome.
    async fn greet(h: &Harness, hello: &Message) -> (Client, WelcomePayload) {
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, hello).await;
        let Message::Welcome(w) = recv_msg(&mut client).await else {
            panic!("expected welcome");
        };
        assert_eq!(w.server_name, "TEST-PC");
        assert_eq!(w.server_id, "test-server");
        (client, w)
    }

    async fn expect_layout(client: &mut Client) {
        assert!(matches!(recv_msg(client).await, Message::LayoutSync(_)));
    }

    /// USB connection, authenticated and past the layout.
    async fn connected(h: &Harness) -> Client {
        let (mut client, welcome) = greet(h, &hello(PROTOCOL_VERSION)).await;
        assert_eq!(welcome.auth, AuthStatus::Authenticated);
        expect_layout(&mut client).await;
        client
    }

    async fn next_event(h: &mut Harness) -> ServerEvent {
        timeout(Duration::from_secs(5), h.events.recv())
            .await
            .unwrap()
            .unwrap()
    }

    fn press(button: &str) -> Message {
        Message::ButtonPress(ButtonEvent {
            button_id: button.into(),
        })
    }

    fn pair_request(pin: &str) -> Message {
        Message::PairRequest(PairRequestPayload { pin: pin.into() })
    }

    fn start_pin(auth: &Auth) -> String {
        let PairingView::Active { pin, .. } = auth.start_pairing().unwrap() else {
            panic!("expected an active PIN");
        };
        pin
    }

    /// Pairs `device_id` over Wi-Fi and returns its token and the open session.
    async fn pair(h: &Harness, device_id: &str) -> (String, Client) {
        let pin = start_pin(&h.auth);
        let (mut client, welcome) = greet(h, &hello_with(PROTOCOL_VERSION, device_id, None)).await;
        assert_eq!(welcome.auth, AuthStatus::PairingRequired);
        send_msg(&mut client, &pair_request(&pin)).await;
        let Message::PairSuccess(success) = recv_msg(&mut client).await else {
            panic!("expected pair:success");
        };
        expect_layout(&mut client).await;
        (success.token, client)
    }

    fn wrong_pin(right: &str) -> String {
        if right == "000000" {
            "111111"
        } else {
            "000000"
        }
        .to_owned()
    }

    // ─── Handshake and session (USB) ──────────────────────────────────────

    #[tokio::test]
    async fn usb_handshake_sends_welcome_then_layout() {
        let mut h = start(Transport::Usb, false).await;
        assert!(matches!(
            next_event(&mut h).await,
            ServerEvent::Listening {
                transport: Transport::Usb,
                ..
            }
        ));
        let _client = connected(&h).await;
        let ServerEvent::ClientConnected {
            device_name,
            device_id,
            transport,
            ..
        } = next_event(&mut h).await
        else {
            panic!("expected ClientConnected");
        };
        assert_eq!(device_name, "Test tablet");
        assert_eq!(device_id, "test-device");
        assert_eq!(transport, Transport::Usb);
    }

    #[tokio::test]
    async fn incompatible_version_is_rejected() {
        let h = start(Transport::Usb, false).await;
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, &hello(PROTOCOL_VERSION + 1)).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::UnsupportedVersion);
        assert_closed(&mut client).await;
    }

    #[tokio::test]
    async fn first_message_must_be_hello() {
        let h = start(Transport::Usb, false).await;
        let (mut client, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 1 })).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::BadRequest);
    }

    #[tokio::test]
    async fn invalid_device_ids_are_rejected() {
        let h = start(Transport::Usb, false).await;
        for id in ["", "a\nb", &"x".repeat(MAX_ID_LEN + 1)] {
            let (mut client, _) = connect_async(&h.url).await.unwrap();
            send_msg(&mut client, &hello_with(PROTOCOL_VERSION, id, None)).await;
            let Message::Error(e) = recv_msg(&mut client).await else {
                panic!("expected error for {id:?}");
            };
            assert_eq!(e.code, ErrorCode::BadRequest);
        }
    }

    #[test]
    fn device_names_are_cleaned() {
        assert_eq!(clean_name("  Tab\u{7}A \n"), "TabA");
        assert_eq!(clean_name(&"é".repeat(100)).chars().count(), MAX_NAME_LEN);
    }

    #[tokio::test]
    async fn press_runs_the_bound_action() {
        let mut h = start(Transport::Usb, false).await;
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("copy")).await;
        loop {
            if let ServerEvent::ButtonPressed {
                button_id, outcome, ..
            } = next_event(&mut h).await
            {
                assert_eq!(button_id, "copy");
                assert_eq!(outcome, ActionOutcome::Ok);
                break;
            }
        }
        assert_eq!(
            *h.executor.executed.lock().unwrap(),
            [Action::Hotkey(Hotkey::new([Key::ControlLeft, Key::KeyC]))]
        );
    }

    #[tokio::test]
    async fn an_edited_layout_is_pushed_and_its_actions_used() {
        let h = start(Transport::Usb, false).await;
        let mut client = connected(&h).await;
        let mut config = crate::config::seed::config();
        config.profiles[0].pages[0].buttons[1].label = "Edited".into();
        config.profiles[0].pages[0].buttons[1].action = Action::Hotkey(Hotkey::new([Key::F14]));
        h.layout.send_replace(Arc::new(config.active_layout()));
        let Message::LayoutSync(layout) = recv_msg(&mut client).await else {
            panic!("expected layout:sync");
        };
        assert_eq!(layout.pages[0].buttons[1].label, "Edited");
        send_msg(&mut client, &press("copy")).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            *h.executor.executed.lock().unwrap(),
            [Action::Hotkey(Hotkey::new([Key::F14]))]
        );
    }

    #[tokio::test]
    async fn state_changes_update_only_the_buttons_they_change() {
        use crate::appearance::Observed;
        use padboard_actions::state::AudioState;
        use padboard_actions::{MicrophoneAction, MicrophoneCommand};

        let h = start(Transport::Usb, false).await;
        // The demo profile with its "copy" button turned into a microphone.
        let mut config = crate::config::seed::config();
        config.profiles[0].pages[0].buttons[1].action = Action::Microphone(MicrophoneAction {
            command: MicrophoneCommand::ToggleMute,
        });
        h.layout.send_replace(Arc::new(config.active_layout()));
        let mut client = connected(&h).await;

        let audio = |muted| {
            Observed::Known(AudioState {
                output_percent: 50,
                output_muted: false,
                input_muted: Some(muted),
            })
        };
        let set = |muted| {
            h.state.send_modify(|s| s.audio = audio(muted));
        };
        set(false);
        // Unmuted: the resting look, nothing to send.
        set(true);
        let Message::ButtonUpdate(update) = recv_msg(&mut client).await else {
            panic!("expected button:update");
        };
        assert_eq!(update.id, "copy");
        assert!(update.active);
        // The sound server repeats itself: nothing more.
        set(true);
        assert!(
            timeout(Duration::from_millis(300), client.next())
                .await
                .is_err(),
            "no update for an unchanged look"
        );

        // A tablet connecting now gets the muted look in its layout.
        let (mut other, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        let Message::LayoutSync(layout) = recv_msg(&mut other).await else {
            panic!("expected layout:sync");
        };
        let copy = layout.pages[0]
            .buttons
            .iter()
            .find(|b| b.id == "copy")
            .unwrap();
        assert!(copy.active);
    }

    #[tokio::test]
    async fn navigation_buttons_move_the_tablet_that_pressed() {
        let h = start(Transport::Usb, false).await;
        let mut config = crate::config::seed::config();
        config.profiles[0].pages[0].buttons[1].action = Action::Navigate(NavigateAction {
            to: NavigateTo::Folder,
            id: Some("f1".into()),
        });
        h.layout.send_replace(Arc::new(config.active_layout()));
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("copy")).await;
        let Message::Navigate(nav) = recv_msg(&mut client).await else {
            panic!("expected navigate");
        };
        assert_eq!(nav.to, NavigateTarget::Folder);
        assert_eq!(nav.id.as_deref(), Some("f1"));
        // Nothing ran on the PC.
        assert!(h.executor.executed.lock().unwrap().is_empty());
    }

    /// The demo profile with its "copy" button bound to `action`.
    fn with_copy_bound_to(h: &Harness, action: Action) {
        let mut config = crate::config::seed::config();
        config.profiles[0].pages[0].buttons[1].action = action;
        h.layout.send_replace(Arc::new(config.active_layout()));
    }

    fn multi(steps: Vec<(Action, u32)>) -> Action {
        Action::Multi(padboard_actions::MultiAction {
            steps: steps
                .into_iter()
                .map(|(action, delay_ms)| padboard_actions::MultiStep { action, delay_ms })
                .collect(),
        })
    }

    fn key(k: Key) -> Action {
        Action::Hotkey(Hotkey::new([k]))
    }

    #[tokio::test]
    async fn multi_actions_run_their_steps_in_order() {
        let h = start(Transport::Usb, false).await;
        let next = Action::Navigate(NavigateAction {
            to: NavigateTo::Next,
            id: None,
        });
        with_copy_bound_to(
            &h,
            multi(vec![(key(Key::F13), 150), (next, 0), (key(Key::F14), 0)]),
        );
        let mut client = connected(&h).await;
        let started = std::time::Instant::now();
        send_msg(&mut client, &press("copy")).await;
        let Message::Navigate(nav) = recv_msg(&mut client).await else {
            panic!("expected navigate");
        };
        assert_eq!(nav.to, NavigateTarget::Next);
        // The pause after the first step was observed.
        assert!(started.elapsed() >= Duration::from_millis(150));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            *h.executor.executed.lock().unwrap(),
            [key(Key::F13), key(Key::F14)]
        );
    }

    #[tokio::test]
    async fn a_failed_step_stops_the_multi_action() {
        let h = start(Transport::Usb, true).await;
        with_copy_bound_to(&h, multi(vec![(key(Key::F13), 0), (key(Key::F14), 0)]));
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("copy")).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::ActionFailed);
        assert!(e.message.starts_with("step 1:"), "{}", e.message);
        assert_eq!(*h.executor.executed.lock().unwrap(), [key(Key::F13)]);
    }

    #[tokio::test]
    async fn a_running_multi_action_ignores_new_presses_but_not_pings() {
        let h = start(Transport::Usb, false).await;
        with_copy_bound_to(&h, multi(vec![(key(Key::F13), 600), (key(Key::F14), 0)]));
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("copy")).await;
        send_msg(&mut client, &press("copy")).await;
        // The session answers while the sequence waits.
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 9 })).await;
        let started = std::time::Instant::now();
        assert_eq!(
            recv_msg(&mut client).await,
            Message::Pong(Heartbeat { seq: 9 })
        );
        assert!(started.elapsed() < Duration::from_millis(300));
        tokio::time::sleep(Duration::from_millis(900)).await;
        assert_eq!(
            *h.executor.executed.lock().unwrap(),
            [key(Key::F13), key(Key::F14)]
        );
        // Done: it can run again.
        send_msg(&mut client, &press("copy")).await;
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert_eq!(h.executor.executed.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn failed_action_is_reported_to_the_tablet() {
        let h = start(Transport::Usb, true).await;
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("copy")).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::ActionFailed);
    }

    #[tokio::test]
    async fn unknown_button_is_reported() {
        let h = start(Transport::Usb, false).await;
        let mut client = connected(&h).await;
        send_msg(&mut client, &press("nope")).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::UnknownButton);
        assert!(h.executor.executed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ping_gets_pong_and_garbage_is_survived() {
        let h = start(Transport::Usb, false).await;
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
        let h = start(Transport::Usb, false).await;
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
    async fn browser_handshake_is_rejected_on_both_transports() {
        for transport in [Transport::Usb, Transport::Wifi] {
            let h = start(transport, false).await;
            let mut request = h.url.as_str().into_client_request().unwrap();
            request
                .headers_mut()
                .insert("origin", "https://evil.example".parse().unwrap());
            let err = connect_async(request).await.unwrap_err();
            let tungstenite::Error::Http(response) = err else {
                panic!("expected an HTTP error, got {err:?}");
            };
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{transport:?}");
        }
    }

    #[tokio::test]
    async fn bind_failure_is_reported() {
        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = taken.local_addr().unwrap();
        let (tx, mut rx) = unbounded_channel();
        let ctx = context(Arc::new(FakeExecutor::default()), tx);
        run(addr, Transport::Usb, None, ctx).await;
        assert!(matches!(
            rx.recv().await,
            Some(ServerEvent::BindFailed { addr: a, transport: Transport::Usb, .. }) if a == addr
        ));
    }

    // ─── TLS (Wi-Fi port) ─────────────────────────────────────────────────

    struct TlsHarness {
        addr: SocketAddr,
        identity: crate::tls::TlsIdentity,
        _config: tempfile::TempDir,
    }

    async fn start_tls() -> TlsHarness {
        let tmp = tempfile::tempdir().unwrap();
        let config = crate::storage::ConfigDir::open(tmp.path()).unwrap();
        let identity = crate::tls::TlsIdentity::load_or_create(&config).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, _events) = unbounded_channel();
        let ctx = context(Arc::new(FakeExecutor::default()), tx);
        tokio::spawn(serve(
            listener,
            Transport::Wifi,
            Some(identity.acceptor.clone()),
            ctx,
        ));
        TlsHarness {
            addr,
            identity,
            _config: tmp,
        }
    }

    /// TLS connection pinned like the tablet, then the WebSocket handshake.
    async fn tls_client(
        addr: SocketAddr,
        pin: crate::pinning::Pin,
    ) -> Result<
        (
            WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>,
            String,
        ),
        String,
    > {
        let connector =
            tokio_rustls::TlsConnector::from(Arc::new(crate::pinning::client_config(pin)));
        let tcp = TcpStream::connect(addr).await.unwrap();
        let name = rustls::pki_types::ServerName::try_from("padboard.local").unwrap();
        let tls = connector
            .connect(name, tcp)
            .await
            .map_err(|e| e.to_string())?;
        let certificate = tls.get_ref().1.peer_certificates().unwrap()[0].clone();
        let (_, code) = crate::pinning::certificate_key(&certificate).unwrap();
        let (ws, _) = tokio_tungstenite::client_async(format!("wss://{addr}"), tls)
            .await
            .map_err(|e| e.to_string())?;
        Ok((ws, code))
    }

    #[tokio::test]
    async fn wifi_speaks_tls_with_the_pinned_key() {
        let h = start_tls().await;
        let (mut ws, code) = tls_client(h.addr, crate::pinning::Pin::TrustOnFirstUse)
            .await
            .unwrap();
        // What the tablet shows is what the desktop shows.
        assert_eq!(code, h.identity.fingerprint);
        ws.send(WsMessage::text(hello(PROTOCOL_VERSION).to_json().unwrap()))
            .await
            .unwrap();
        let frame = timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Welcome(w) = Message::from_json(frame.to_text().unwrap()).unwrap() else {
            panic!("expected welcome");
        };
        assert_eq!(w.auth, AuthStatus::PairingRequired);

        // Pinned to the right key: accepted.
        let pin = crate::pinning::Pin::Spki(h.identity.spki_sha256);
        assert!(tls_client(h.addr, pin).await.is_ok());
    }

    #[tokio::test]
    async fn another_key_is_refused_by_a_pinned_client() {
        let h = start_tls().await;
        let err = tls_client(h.addr, crate::pinning::Pin::Spki([0; 32]))
            .await
            .unwrap_err();
        assert!(err.contains("does not match the pin"), "{err}");
    }

    #[tokio::test]
    async fn plain_websocket_is_refused_on_the_tls_port() {
        let h = start_tls().await;
        let result = timeout(
            Duration::from_secs(10),
            connect_async(format!("ws://{}", h.addr)),
        )
        .await
        .expect("plain connection attempt hung");
        assert!(result.is_err());
    }

    // ─── Wi-Fi authentication ─────────────────────────────────────────────

    #[tokio::test]
    async fn wifi_without_token_may_only_pair() {
        let h = start(Transport::Wifi, false).await;
        let (mut client, welcome) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        assert_eq!(welcome.auth, AuthStatus::PairingRequired);
        send_msg(&mut client, &press("copy")).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Unauthorized);
        assert_closed(&mut client).await;
        assert!(h.executor.executed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn even_a_ping_is_refused_before_authentication() {
        let h = start(Transport::Wifi, false).await;
        let (mut client, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 1 })).await;
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Unauthorized);
        assert_closed(&mut client).await;
    }

    #[tokio::test]
    async fn pairing_with_the_pin_authenticates_and_issues_a_token() {
        let mut h = start(Transport::Wifi, false).await;
        let pin = start_pin(&h.auth);
        let (mut client, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;

        send_msg(&mut client, &pair_request(&wrong_pin(&pin))).await;
        let Message::PairFailure(failure) = recv_msg(&mut client).await else {
            panic!("expected pair:failure");
        };
        assert_eq!(failure.reason, PairFailureReason::InvalidPin);
        assert_eq!(failure.attempts_left, 4);

        send_msg(&mut client, &pair_request(&pin)).await;
        let Message::PairSuccess(success) = recv_msg(&mut client).await else {
            panic!("expected pair:success");
        };
        expect_layout(&mut client).await;
        send_msg(&mut client, &press("copy")).await;
        loop {
            if let ServerEvent::ButtonPressed { outcome, .. } = next_event(&mut h).await {
                assert_eq!(outcome, ActionOutcome::Ok);
                break;
            }
        }
        drop(client);

        // The token authenticates the next connections directly.
        let (mut again, welcome) = greet(
            &h,
            &hello_with(PROTOCOL_VERSION, "test-device", Some(&success.token)),
        )
        .await;
        assert_eq!(welcome.auth, AuthStatus::Authenticated);
        expect_layout(&mut again).await;
    }

    #[tokio::test]
    async fn the_pin_locks_after_five_wrong_attempts_across_connections() {
        let h = start(Transport::Wifi, false).await;
        let pin = start_pin(&h.auth);
        let wrong = wrong_pin(&pin);
        let mut last = None;
        // Spread over several connections: the limit is per PIN, not per socket.
        for _ in 0..3 {
            let (mut client, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
            for _ in 0..2 {
                send_msg(&mut client, &pair_request(&wrong)).await;
                if let Message::PairFailure(f) = recv_msg(&mut client).await {
                    last = Some(f);
                }
            }
        }
        let last = last.unwrap();
        assert_eq!(last.reason, PairFailureReason::Locked);
        assert_eq!(last.attempts_left, 0);
        assert_eq!(h.auth.pairing(), PairingView::Locked);

        let (mut client, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        send_msg(&mut client, &pair_request(&pin)).await;
        let Message::PairFailure(f) = recv_msg(&mut client).await else {
            panic!("the right PIN must be refused once locked");
        };
        assert_eq!(f.reason, PairFailureReason::Locked);
    }

    #[tokio::test]
    async fn unknown_tokens_are_rejected_but_pairing_stays_possible() {
        let h = start(Transport::Wifi, false).await;
        let (mut client, welcome) = greet(
            &h,
            &hello_with(PROTOCOL_VERSION, "test-device", Some("forged")),
        )
        .await;
        assert_eq!(welcome.auth, AuthStatus::TokenRejected);
        send_msg(&mut client, &pair_request("123456")).await;
        let Message::PairFailure(f) = recv_msg(&mut client).await else {
            panic!("expected pair:failure");
        };
        assert_eq!(f.reason, PairFailureReason::NoActivePin);
    }

    #[tokio::test]
    async fn a_token_only_works_for_its_device() {
        let h = start(Transport::Wifi, false).await;
        let (token, _client) = pair(&h, "tab-a").await;
        let (_, welcome) = greet(&h, &hello_with(PROTOCOL_VERSION, "tab-b", Some(&token))).await;
        assert_eq!(welcome.auth, AuthStatus::TokenRejected);
    }

    #[tokio::test]
    async fn revoking_closes_the_live_session() {
        let h = start(Transport::Wifi, false).await;
        let (token, mut client) = pair(&h, "tab").await;
        assert!(h.auth.revoke("tab"));
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Revoked);
        assert_closed(&mut client).await;

        let (_, welcome) = greet(&h, &hello_with(PROTOCOL_VERSION, "tab", Some(&token))).await;
        assert_eq!(welcome.auth, AuthStatus::TokenRejected);
    }

    #[tokio::test]
    async fn a_new_connection_supersedes_the_old_one() {
        let h = start(Transport::Usb, false).await;
        let mut first = connected(&h).await;
        let mut second = connected(&h).await;
        let Message::Error(e) = recv_msg(&mut first).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Superseded);
        assert_closed(&mut first).await;
        send_msg(&mut second, &Message::Ping(Heartbeat { seq: 3 })).await;
        assert_eq!(
            recv_msg(&mut second).await,
            Message::Pong(Heartbeat { seq: 3 })
        );
    }

    #[tokio::test]
    async fn silent_sessions_are_closed() {
        let mut h = start(Transport::Usb, false).await;
        let mut client = connected(&h).await;
        // Limits::idle is 2 s in tests; a ping keeps the session alive.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 1 })).await;
        assert!(matches!(recv_msg(&mut client).await, Message::Pong(_)));
        tokio::time::sleep(Duration::from_millis(1200)).await;
        send_msg(&mut client, &Message::Ping(Heartbeat { seq: 2 })).await;
        assert!(matches!(recv_msg(&mut client).await, Message::Pong(_)));
        assert_closed(&mut client).await;
        loop {
            if matches!(
                next_event(&mut h).await,
                ServerEvent::ClientDisconnected { .. }
            ) {
                break;
            }
        }
    }

    #[tokio::test]
    async fn unauthenticated_connections_time_out() {
        let h = start(Transport::Wifi, false).await;
        let (mut client, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        // Limits::unauthenticated is 500 ms in tests.
        let Message::Error(e) = recv_msg(&mut client).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Unauthorized);
        assert_closed(&mut client).await;
    }

    #[tokio::test]
    async fn concurrent_pairing_attempts_are_limited() {
        let h = start(Transport::Wifi, false).await;
        // max_pairing is 2 in tests.
        let (a, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        let (_b, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        let (mut c, _) = connect_async(&h.url).await.unwrap();
        send_msg(&mut c, &hello(PROTOCOL_VERSION)).await;
        let Message::Error(e) = recv_msg(&mut c).await else {
            panic!("expected error");
        };
        assert_eq!(e.code, ErrorCode::Unauthorized);
        assert_closed(&mut c).await;

        // A slot frees up once a pairing connection ends.
        drop(a);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (_d, welcome) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        assert_eq!(welcome.auth, AuthStatus::PairingRequired);
    }

    #[tokio::test]
    async fn connections_without_hello_are_limited() {
        let h = start(Transport::Wifi, false).await;
        // max_handshaking is 3 in tests.
        let mut silent = Vec::new();
        for _ in 0..3 {
            silent.push(connect_async(&h.url).await.unwrap().0);
        }
        // The next TCP connection is dropped before the WebSocket handshake.
        assert!(connect_async(&h.url).await.is_err());
    }

    #[tokio::test]
    async fn silent_tcp_connections_are_dropped() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, _events) = unbounded_channel();
        let mut ctx = context(Arc::new(FakeExecutor::default()), tx);
        Arc::get_mut(&mut ctx).unwrap().limits.handshake = Duration::from_millis(200);
        tokio::spawn(serve(listener, Transport::Wifi, None, ctx));
        // Raw TCP, no HTTP upgrade: the server must hang up after the timeout.
        let mut raw = TcpStream::connect(addr).await.unwrap();
        let mut buf = [0u8; 1];
        let read = timeout(
            Duration::from_secs(5),
            tokio::io::AsyncReadExt::read(&mut raw, &mut buf),
        )
        .await
        .expect("the server kept the silent connection open");
        assert!(matches!(read, Ok(0) | Err(_)));
    }

    #[tokio::test]
    async fn authenticated_connections_do_not_count_against_the_limit() {
        let h = start(Transport::Wifi, false).await;
        let (token, _first) = pair(&h, "tab-a").await;
        let (_second, _) = pair(&h, "tab-b").await;
        let (_unauth1, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        let (_unauth2, _) = greet(&h, &hello(PROTOCOL_VERSION)).await;
        let (_, welcome) = greet(&h, &hello_with(PROTOCOL_VERSION, "tab-a", Some(&token))).await;
        assert_eq!(welcome.auth, AuthStatus::Authenticated);
    }
}
