//! OBS Studio through obs-websocket v5 (built into OBS 28+).
//!
//! A small client of our own (the protocol is JSON over WebSocket: `Hello`,
//! `Identify` with a SHA-256 challenge, requests and events) on the
//! WebSocket stack the app already has. One task owns the connection: it
//! connects when OBS is enabled in the settings, retries every few seconds
//! while OBS is closed, waits for OBS to finish starting before asking
//! anything (see [`session`]), publishes what the buttons show (current scene,
//! recording, streaming, muted inputs) and runs the requests of OBS buttons.
//! While it is not connected, OBS buttons are unavailable (dimmed).

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures_util::{SinkExt, StreamExt};
use padboard_actions::{ObsAction, ObsCommand};
use padboard_protocol::REDACTED;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use crate::appearance::{Observed, SystemState};
use crate::storage::Document;

/// Default port of obs-websocket.
pub const DEFAULT_PORT: u16 = 4455;
#[cfg(not(test))]
const RETRY: Duration = Duration::from_secs(3);
#[cfg(test)]
const RETRY: Duration = Duration::from_millis(100);
/// Between OBS having loaded its scenes and the real session (issue #14):
/// OBS then finishes starting, which clients cannot see.
#[cfg(not(test))]
const STARTUP_SETTLE: Duration = Duration::from_secs(10);
#[cfg(test)]
const STARTUP_SETTLE: Duration = Duration::from_millis(500);
/// obs-websocket's `NotReady` request status.
const NOT_READY: u64 = 207;
const TIMEOUT: Duration = Duration::from_secs(3);
/// How long a closing connection waits for OBS's own close frame.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// Config (scene collection changes), scenes, inputs and outputs events.
const EVENT_SUBSCRIPTIONS: u32 = 2 | 4 | 8 | 64;
/// What the state starts from; asked again when the scene collection
/// changes (its scenes and inputs replace the previous ones).
const INITIAL_REQUESTS: [&str; 5] = [
    "GetCurrentProgramScene",
    "GetRecordStatus",
    "GetStreamStatus",
    "GetSceneList",
    "GetInputList",
];
const COLLECTION_REQUESTS: [&str; 3] = ["GetCurrentProgramScene", "GetSceneList", "GetInputList"];

/// `obs.json` (owner-only: it holds the obs-websocket password). `Debug`
/// redacts the password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl std::fmt::Debug for ObsSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsSettings")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("password", &self.password.as_ref().map(|_| REDACTED))
            .finish()
    }
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".to_owned(),
            port: DEFAULT_PORT,
            password: None,
        }
    }
}

impl Document for ObsSettings {
    const FILE: &'static str = "obs.json";
    const VERSION: u32 = 1;
    const PRIVATE: bool = true;
}

/// What OBS buttons show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObsState {
    pub scene: Option<String>,
    pub recording: bool,
    pub streaming: bool,
    pub muted_inputs: BTreeSet<String>,
    /// For the editor's pickers.
    pub scenes: Vec<String>,
    pub inputs: Vec<String>,
}

/// Connection state, for the connections view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ObsStatus {
    Disabled,
    Connecting,
    Connected {
        version: String,
    },
    /// OBS closed, or obs-websocket off: retried every few seconds.
    Unreachable {
        detail: String,
    },
    /// Wrong password: retried when the settings change.
    AuthFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum ObsError {
    #[error("OBS is not connected")]
    NotConnected,
    #[error("OBS refused: {0}")]
    Refused(String),
    #[error("{0}")]
    Connection(String),
    #[error("wrong obs-websocket password")]
    Auth,
}

type Reply = oneshot::Sender<Result<Value, ObsError>>;

/// Runs OBS requests and follows the connection. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Obs {
    requests: mpsc::Sender<(String, Value, Reply)>,
    settings: watch::Sender<ObsSettings>,
    status: watch::Receiver<ObsStatus>,
}

impl Obs {
    /// Starts the connection task. `state` gets the observed OBS state.
    #[must_use]
    pub fn start(settings: ObsSettings, state: watch::Sender<SystemState>) -> Self {
        let (requests, requests_rx) = mpsc::channel(16);
        let (settings_tx, settings_rx) = watch::channel(settings);
        let (status_tx, status) = watch::channel(ObsStatus::Disabled);
        tauri::async_runtime::spawn(run(settings_rx, requests_rx, state, status_tx));
        Self {
            requests,
            settings: settings_tx,
            status,
        }
    }

    pub fn settings(&self) -> ObsSettings {
        self.settings.borrow().clone()
    }

    /// New settings: the connection restarts with them.
    pub fn set_settings(&self, settings: ObsSettings) {
        self.settings.send_replace(settings);
    }

    pub fn status(&self) -> watch::Receiver<ObsStatus> {
        self.status.clone()
    }

    /// Runs the request of an OBS button.
    ///
    /// # Errors
    /// [`ObsError::NotConnected`] while OBS is closed; [`ObsError::Refused`]
    /// when OBS refuses (unknown scene…).
    pub async fn perform(&self, action: &ObsAction) -> Result<(), ObsError> {
        let target = action.target.clone().unwrap_or_default();
        let (request, data) = match action.command {
            ObsCommand::SetScene => ("SetCurrentProgramScene", json!({ "sceneName": target })),
            ObsCommand::ToggleRecord => ("ToggleRecord", json!({})),
            ObsCommand::ToggleStream => ("ToggleStream", json!({})),
            ObsCommand::ToggleInputMute => ("ToggleInputMute", json!({ "inputName": target })),
        };
        self.request(request, data).await.map(drop)
    }

    async fn request(&self, request: &str, data: Value) -> Result<Value, ObsError> {
        let (reply, answer) = oneshot::channel();
        self.requests
            .send((request.to_owned(), data, reply))
            .await
            .map_err(|_| ObsError::NotConnected)?;
        tokio::time::timeout(TIMEOUT, answer)
            .await
            .map_err(|_| ObsError::Connection("OBS did not answer".into()))?
            .map_err(|_| ObsError::NotConnected)?
    }
}

/// The `authentication` string of `Identify`.
#[must_use]
pub fn authentication(password: &str, salt: &str, challenge: &str) -> String {
    let secret = BASE64.encode(Sha256::digest(format!("{password}{salt}")));
    BASE64.encode(Sha256::digest(format!("{secret}{challenge}")))
}

fn publish(state: &watch::Sender<SystemState>, obs: Observed<ObsState>) {
    state.send_if_modified(|s| {
        let changed = s.obs != obs;
        s.obs = obs;
        changed
    });
}

async fn run(
    mut settings: watch::Receiver<ObsSettings>,
    mut requests: mpsc::Receiver<(String, Value, Reply)>,
    state: watch::Sender<SystemState>,
    status: watch::Sender<ObsStatus>,
) {
    // OBS was closed or loading at the last attempt: when it answers, it is
    // starting (see `Outcome::Starting`). Not known at launch: OBS may have
    // been running for hours.
    let mut starting = false;
    // What the last attempt found, so a retry finding the same is logged at
    // DEBUG only (OBS can stay closed all day).
    let mut last = String::new();
    loop {
        let current = settings.borrow_and_update().clone();
        if !current.enabled {
            status.send_replace(ObsStatus::Disabled);
            publish(&state, Observed::Unavailable);
            last.clear();
            tokio::select! {
                changed = settings.changed() => if changed.is_err() { return },
                Some((_, _, reply)) = requests.recv() => { let _ = reply.send(Err(ObsError::NotConnected)); }
            }
            continue;
        }
        status.send_replace(ObsStatus::Connecting);
        let outcome = session(
            &current,
            starting,
            &mut settings,
            &mut requests,
            &state,
            &status,
            &mut last,
        )
        .await;
        publish(&state, Observed::Unavailable);
        // Logged once per lost connection, not at each retry while OBS is
        // closed.
        let was_connected = matches!(*status.borrow(), ObsStatus::Connected { .. });
        let wait = match outcome {
            Ok(Outcome::SettingsChanged) => {
                if was_connected {
                    tracing::info!("OBS settings changed: reconnecting");
                }
                last.clear();
                continue;
            }
            Ok(Outcome::Loading) => {
                starting = true;
                note(
                    &mut last,
                    format!(
                        "OBS is still loading (207 not ready): disconnected, trying again in {} s",
                        RETRY.as_secs()
                    ),
                );
                status.send_replace(ObsStatus::Connecting);
                RETRY
            }
            Ok(Outcome::Starting) => {
                starting = false;
                note(
                    &mut last,
                    format!(
                        "OBS has loaded its scenes: waiting {} s for it to finish starting",
                        STARTUP_SETTLE.as_secs()
                    ),
                );
                status.send_replace(ObsStatus::Connecting);
                STARTUP_SETTLE
            }
            Err(ObsError::Auth) => {
                tracing::warn!("OBS refused the password");
                last.clear();
                status.send_replace(ObsStatus::AuthFailed);
                // Retrying would fail the same way: wait for new settings.
                loop {
                    tokio::select! {
                        changed = settings.changed() => { if changed.is_err() { return } break }
                        Some((_, _, reply)) = requests.recv() => { let _ = reply.send(Err(ObsError::NotConnected)); }
                    }
                }
                continue;
            }
            Err(e) => {
                // Closed, or closing: when it answers again, it is starting.
                starting = true;
                if was_connected {
                    tracing::warn!("disconnected from OBS: {e}");
                    last.clear();
                } else {
                    note(
                        &mut last,
                        format!(
                            "OBS not reachable: {e} (trying again every {} s)",
                            RETRY.as_secs()
                        ),
                    );
                }
                status.send_replace(ObsStatus::Unreachable {
                    detail: e.to_string(),
                });
                RETRY
            }
        };
        let retry = tokio::time::sleep(wait);
        tokio::pin!(retry);
        loop {
            tokio::select! {
                () = &mut retry => break,
                changed = settings.changed() => { if changed.is_err() { return } break }
                Some((_, _, reply)) = requests.recv() => { let _ = reply.send(Err(ObsError::NotConnected)); }
            }
        }
    }
}

/// Logs `line` at INFO, or at DEBUG when the previous attempt found the same.
fn note(last: &mut String, line: String) {
    if *last == line {
        tracing::debug!("{line}");
    } else {
        tracing::info!("{line}");
        *last = line;
    }
}

/// How a session ended, short of an error.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// New settings: reconnect at once.
    SettingsChanged,
    /// OBS answers 207: it has not loaded its scene collection yet.
    Loading,
    /// OBS was seen closed or loading before, and now answers: it has just
    /// loaded its scenes but has not finished starting.
    Starting,
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn recv_json(ws: &mut Ws) -> Result<Value, ObsError> {
    loop {
        let frame = ws
            .next()
            .await
            .ok_or_else(|| ObsError::Connection("OBS closed the connection".into()))?
            .map_err(|e| ObsError::Connection(e.to_string()))?;
        match frame {
            WsMessage::Text(text) => {
                return serde_json::from_str(&text)
                    .map_err(|e| ObsError::Connection(e.to_string()));
            }
            WsMessage::Close(frame) => {
                // 4009: authentication failed.
                if frame.as_ref().is_some_and(|f| u16::from(f.code) == 4009) {
                    return Err(ObsError::Auth);
                }
                return Err(ObsError::Connection("OBS closed the connection".into()));
            }
            _ => {}
        }
    }
}

/// Closes the connection as OBS expects: a close frame with 1000 (normal
/// closure), then OBS's own close frame read before the socket is dropped
/// (at most [`CLOSE_TIMEOUT`]). Dropping it right after sending the close
/// made OBS log 1006, "End of File", for every check connection (#15).
async fn close(mut ws: Ws) {
    let frame = CloseFrame {
        code: CloseCode::Normal,
        reason: "".into(),
    };
    if ws.close(Some(frame)).await.is_err() {
        return;
    }
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, async {
        while let Some(Ok(_)) = ws.next().await {}
    })
    .await;
}

async fn send_json(ws: &mut Ws, value: &Value) -> Result<(), ObsError> {
    ws.send(WsMessage::text(value.to_string()))
        .await
        .map_err(|e| ObsError::Connection(e.to_string()))
}

/// Hello → Identify → Identified, subscribed to `subscriptions`; returns the
/// obs-websocket version.
async fn handshake(
    ws: &mut Ws,
    settings: &ObsSettings,
    subscriptions: u32,
) -> Result<String, ObsError> {
    let hello = tokio::time::timeout(TIMEOUT, recv_json(ws))
        .await
        .map_err(|_| ObsError::Connection("no Hello from OBS".into()))??;
    let version = hello["d"]["obsWebSocketVersion"]
        .as_str()
        .unwrap_or("?")
        .to_owned();
    let mut identify = json!({ "rpcVersion": 1, "eventSubscriptions": subscriptions });
    if let Some(auth) = hello["d"].get("authentication") {
        let password = settings.password.as_deref().ok_or(ObsError::Auth)?;
        identify["authentication"] = json!(authentication(
            password,
            auth["salt"].as_str().unwrap_or_default(),
            auth["challenge"].as_str().unwrap_or_default(),
        ));
    }
    send_json(ws, &json!({ "op": 1, "d": identify })).await?;
    let identified = tokio::time::timeout(TIMEOUT, recv_json(ws))
        .await
        .map_err(|_| ObsError::Connection("OBS did not identify us".into()))??;
    if identified["op"] != 2 {
        return Err(ObsError::Connection(format!(
            "unexpected answer: {identified}"
        )));
    }
    Ok(version)
}

/// Asks `GetVersion`, which needs nothing from OBS's interface: `false` if
/// OBS answers 207 (scene collection not loaded yet).
async fn obs_ready(ws: &mut Ws) -> Result<bool, ObsError> {
    send_json(
        ws,
        &json!({ "op": 6, "d": { "requestType": "GetVersion", "requestId": "probe", "requestData": {} } }),
    )
    .await?;
    let answer = tokio::time::timeout(TIMEOUT, async {
        loop {
            let message = recv_json(ws).await?;
            if message["op"] == 7 && message["d"]["requestId"] == "probe" {
                return Ok::<_, ObsError>(message);
            }
        }
    })
    .await
    .map_err(|_| ObsError::Connection("OBS did not answer GetVersion".into()))??;
    let status = &answer["d"]["requestStatus"];
    match (status["result"].as_bool(), status["code"].as_u64()) {
        (Some(true), _) => Ok(true),
        (_, Some(NOT_READY)) => Ok(false),
        _ => Err(ObsError::Connection(format!(
            "GetVersion refused: {}",
            status["comment"].as_str().unwrap_or("?")
        ))),
    }
}

/// Connects and follows OBS until the connection ends (`Err`), the settings
/// change, or OBS turns out to be starting.
///
/// obs-websocket accepts clients and requests while OBS is still starting:
/// it answers 207 until the scene collection is loaded, then serves requests
/// while OBS's interface thread is still in its initialisation (until
/// `OBS_FRONTEND_EVENT_FINISHED_LOADING`, of which clients hear nothing).
/// Requests in that window can deadlock OBS on Windows (issue #14). So a
/// session never stays while OBS answers 207, and when OBS was closed or
/// loading at the last attempt (`starting`) this one only checks, subscribed
/// to nothing, that it has loaded its scenes ([`Outcome::Starting`]): the
/// real session comes [`STARTUP_SETTLE`] later.
#[allow(clippy::too_many_lines)]
async fn session(
    settings: &ObsSettings,
    starting: bool,
    settings_rx: &mut watch::Receiver<ObsSettings>,
    requests: &mut mpsc::Receiver<(String, Value, Reply)>,
    state: &watch::Sender<SystemState>,
    status: &watch::Sender<ObsStatus>,
    last: &mut String,
) -> Result<Outcome, ObsError> {
    let url = format!("ws://{}:{}", settings.host, settings.port);
    let attempt = if starting {
        format!("connecting to OBS at {url} (only checking that it has loaded)")
    } else {
        format!("connecting to OBS at {url}")
    };
    if last.starts_with("OBS not reachable") {
        tracing::debug!("{attempt}");
    } else {
        tracing::info!("{attempt}");
    }
    let (mut ws, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::connect_async(&url))
        .await
        .map_err(|_| ObsError::Connection(format!("no answer from {url}")))?
        .map_err(|e| ObsError::Connection(format!("{url}: {e}")))?;
    let subscriptions = if starting { 0 } else { EVENT_SUBSCRIPTIONS };
    let version = handshake(&mut ws, settings, subscriptions).await?;
    let ready = obs_ready(&mut ws).await?;
    if !ready || starting {
        close(ws).await;
        return Ok(if ready {
            Outcome::Starting
        } else {
            Outcome::Loading
        });
    }
    tracing::info!(%version, "connected to OBS");
    last.clear();
    status.send_replace(ObsStatus::Connected { version });

    let mut next_id: u64 = 0;
    let mut pending: HashMap<String, Reply> = HashMap::new();
    // The last batch of GetInputMute and its inputs, in order (an older
    // batch, from before a scene collection change, is ignored).
    let mut mute_batch: Option<(String, Vec<String>)> = None;
    let mut obs = ObsState::default();

    // Initial state: answers are collected below like any reply.
    for request in INITIAL_REQUESTS {
        next_id += 1;
        send_initial(&mut ws, request, next_id).await?;
    }
    // Answers can come in any order: the state is published once all the
    // initial ones are in (no half-filled state on the buttons).
    let mut initial_left = INITIAL_REQUESTS.len();
    let mut ready = false;

    loop {
        tokio::select! {
            message = recv_json(&mut ws) => {
                let message = message?;
                let d = &message["d"];
                match message["op"].as_u64() {
                    // RequestResponse
                    Some(7) => {
                        let id = d["requestId"].as_str().unwrap_or_default().to_owned();
                        if let Some(reply) = pending.remove(&id) {
                            let ok = d["requestStatus"]["result"].as_bool().unwrap_or(false);
                            let _ = reply.send(if ok {
                                Ok(d["responseData"].clone())
                            } else {
                                Err(ObsError::Refused(
                                    d["requestStatus"]["comment"].as_str().unwrap_or("refused").to_owned(),
                                ))
                            });
                        } else if id.starts_with("init-") {
                            let request = d["requestType"].as_str().unwrap_or_default();
                            if d["requestStatus"]["result"].as_bool() == Some(true) {
                                tracing::info!(request, "OBS answered");
                            } else {
                                // The state stays without it (an empty list…).
                                tracing::warn!(
                                    request,
                                    code = d["requestStatus"]["code"].as_u64(),
                                    comment = d["requestStatus"]["comment"].as_str().unwrap_or_default(),
                                    "OBS refused an initial request"
                                );
                            }
                            let inputs = apply_initial(&mut obs, d);
                            if !inputs.is_empty() {
                                next_id += 1;
                                let id = format!("mutes-{next_id}");
                                tracing::info!(inputs = inputs.len(), "OBS request GetInputMute (one batch)");
                                send_json(&mut ws, &mute_request(&id, &inputs)).await?;
                                mute_batch = Some((id, inputs));
                            }
                            initial_left = initial_left.saturating_sub(1);
                            ready = initial_left == 0;
                            if ready {
                                tracing::info!(
                                    scenes = obs.scenes.len(),
                                    inputs = obs.inputs.len(),
                                    "OBS state received"
                                );
                                publish(state, Observed::Known(obs.clone()));
                            }
                        }
                    }
                    // RequestBatchResponse
                    Some(9) => {
                        let Some((id, inputs)) = mute_batch.take_if(|(id, _)| d["requestId"] == id.as_str()) else {
                            continue;
                        };
                        let (refused, why) = apply_mutes(&mut obs, &inputs, &d["results"]);
                        tracing::info!(
                            inputs = inputs.len(),
                            muted = obs.muted_inputs.len(),
                            refused,
                            first_refusal = why.as_deref().unwrap_or("none"),
                            batch = id,
                            "OBS answered GetInputMute"
                        );
                        if ready {
                            publish(state, Observed::Known(obs.clone()));
                        }
                    }
                    // Another scene collection: its scenes and inputs are
                    // asked again (answers handled like the initial ones).
                    Some(5) if d["eventType"] == "CurrentSceneCollectionChanged" => {
                        tracing::info!("OBS scene collection changed: asking its scenes and inputs");
                        for request in COLLECTION_REQUESTS {
                            next_id += 1;
                            send_initial(&mut ws, request, next_id).await?;
                        }
                    }
                    // Event
                    Some(5) if apply_event(&mut obs, d) && ready => {
                        publish(state, Observed::Known(obs.clone()));
                    }
                    _ => {}
                }
            }
            Some((request, data, reply)) = requests.recv() => {
                next_id += 1;
                let id = format!("req-{next_id}");
                send_json(&mut ws, &json!({ "op": 6, "d": { "requestType": request, "requestId": id, "requestData": data } })).await?;
                pending.insert(id, reply);
            }
            changed = settings_rx.changed() => {
                close(ws).await;
                return if changed.is_ok() { Ok(Outcome::SettingsChanged) } else { Err(ObsError::NotConnected) };
            }
        }
    }
}

/// One `RequestBatch` asking the mute state of `inputs`, run one after the
/// other by OBS (`SerialRealtime`).
fn mute_request(id: &str, inputs: &[String]) -> Value {
    let requests: Vec<Value> = inputs
        .iter()
        .map(
            |input| json!({ "requestType": "GetInputMute", "requestData": { "inputName": input } }),
        )
        .collect();
    json!({ "op": 8, "d": {
        "requestId": id,
        "haltOnFailure": false,
        "executionType": 0,
        "requests": requests,
    }})
}

/// Applies the results of [`mute_request`] (in the order of `inputs`);
/// returns how many OBS refused, and why for the first one (an input with
/// no audio, such as a color source, is refused).
fn apply_mutes(obs: &mut ObsState, inputs: &[String], results: &Value) -> (usize, Option<String>) {
    let results = results.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut refused = inputs.len().saturating_sub(results.len());
    let mut why = None;
    for (input, result) in inputs.iter().zip(results) {
        if result["requestStatus"]["result"].as_bool() != Some(true) {
            refused += 1;
            why.get_or_insert_with(|| {
                format!(
                    "{input}: {}",
                    result["requestStatus"]["comment"].as_str().unwrap_or("?")
                )
            });
        } else if result["responseData"]["inputMuted"].as_bool() == Some(true) {
            obs.muted_inputs.insert(input.clone());
        }
    }
    (refused, why)
}

/// Sends a request whose answer [`apply_initial`] reads.
async fn send_initial(ws: &mut Ws, request: &str, id: u64) -> Result<(), ObsError> {
    tracing::info!(request, "OBS request");
    send_json(
        ws,
        &json!({ "op": 6, "d": {
            "requestType": request,
            "requestId": format!("init-{request}-{id}"),
            "requestData": {},
        }}),
    )
    .await
}

/// Applies the answer of an initial request; returns the inputs whose mute
/// state must be asked.
fn apply_initial(obs: &mut ObsState, d: &Value) -> Vec<String> {
    let data = &d["responseData"];
    match d["requestType"].as_str() {
        Some("GetCurrentProgramScene") => {
            obs.scene = data["currentProgramSceneName"]
                .as_str()
                .or_else(|| data["sceneName"].as_str())
                .map(str::to_owned);
        }
        Some("GetRecordStatus") => obs.recording = data["outputActive"].as_bool().unwrap_or(false),
        Some("GetStreamStatus") => obs.streaming = data["outputActive"].as_bool().unwrap_or(false),
        Some("GetSceneList") => {
            // OBS lists scenes bottom-up; the editor shows them as in OBS.
            obs.scenes = names(&data["scenes"], "sceneName");
            obs.scenes.reverse();
        }
        Some("GetInputList") => {
            obs.inputs = names(&data["inputs"], "inputName");
            // Asked again for each input.
            obs.muted_inputs.clear();
            return obs.inputs.clone();
        }
        _ => {}
    }
    Vec::new()
}

fn names(list: &Value, key: &str) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item[key].as_str().map(str::to_owned))
        .collect()
}

/// Applies an event; `true` if the state changed.
fn apply_event(obs: &mut ObsState, d: &Value) -> bool {
    let data = &d["eventData"];
    let before = obs.clone();
    match d["eventType"].as_str() {
        Some("CurrentProgramSceneChanged") => {
            obs.scene = data["sceneName"].as_str().map(str::to_owned)
        }
        Some("RecordStateChanged") => {
            obs.recording = data["outputActive"].as_bool().unwrap_or(false)
        }
        Some("StreamStateChanged") => {
            obs.streaming = data["outputActive"].as_bool().unwrap_or(false)
        }
        Some("InputMuteStateChanged") => {
            if let Some(name) = data["inputName"].as_str() {
                if data["inputMuted"].as_bool().unwrap_or(false) {
                    obs.muted_inputs.insert(name.to_owned());
                } else {
                    obs.muted_inputs.remove(name);
                }
            }
        }
        Some("SceneCreated") => {
            if let Some(name) = data["sceneName"].as_str() {
                obs.scenes.push(name.to_owned());
            }
        }
        // Scenes reordered: the whole list, like GetSceneList.
        Some("SceneListChanged") => {
            obs.scenes = names(&data["scenes"], "sceneName");
            obs.scenes.reverse();
        }
        Some("SceneRemoved") => obs
            .scenes
            .retain(|s| Some(s.as_str()) != data["sceneName"].as_str()),
        Some("SceneNameChanged") => {
            if let (Some(old), Some(new)) =
                (data["oldSceneName"].as_str(), data["sceneName"].as_str())
            {
                for s in &mut obs.scenes {
                    if s == old {
                        new.clone_into(s);
                    }
                }
                if obs.scene.as_deref() == Some(old) {
                    obs.scene = Some(new.to_owned());
                }
            }
        }
        Some("InputCreated") => {
            if let Some(name) = data["inputName"].as_str() {
                obs.inputs.push(name.to_owned());
            }
        }
        Some("InputRemoved") => {
            let name = data["inputName"].as_str();
            obs.inputs.retain(|i| Some(i.as_str()) != name);
            if let Some(name) = name {
                obs.muted_inputs.remove(name);
            }
        }
        Some("InputNameChanged") => {
            if let (Some(old), Some(new)) =
                (data["oldInputName"].as_str(), data["inputName"].as_str())
            {
                for i in &mut obs.inputs {
                    if i == old {
                        new.clone_into(i);
                    }
                }
                if obs.muted_inputs.remove(old) {
                    obs.muted_inputs.insert(new.to_owned());
                }
            }
        }
        _ => {}
    }
    *obs != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_matches_the_obs_websocket_documentation() {
        // Example of the obs-websocket v5 protocol documentation.
        assert_eq!(
            authentication(
                "supersecretpassword",
                "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=",
                "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="
            ),
            "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4="
        );
    }

    #[test]
    fn debug_redacts_the_password() {
        let settings = ObsSettings {
            password: Some("obs-SECRET".into()),
            ..ObsSettings::default()
        };
        for text in [format!("{settings:?}"), format!("{settings:#?}")] {
            assert!(!text.contains("SECRET"), "{text}");
            assert!(
                text.contains(REDACTED) && text.contains("127.0.0.1"),
                "{text}"
            );
        }
    }

    #[test]
    fn events_update_the_state() {
        let mut obs = ObsState {
            scenes: vec!["Jeu".into()],
            inputs: vec!["Mic/Aux".into()],
            ..ObsState::default()
        };
        let event = |kind: &str, data: Value| json!({ "eventType": kind, "eventData": data });
        assert!(apply_event(
            &mut obs,
            &event("CurrentProgramSceneChanged", json!({"sceneName": "Jeu"}))
        ));
        assert_eq!(obs.scene.as_deref(), Some("Jeu"));
        assert!(apply_event(
            &mut obs,
            &event("RecordStateChanged", json!({"outputActive": true}))
        ));
        assert!(obs.recording);
        assert!(apply_event(
            &mut obs,
            &event(
                "InputMuteStateChanged",
                json!({"inputName": "Mic/Aux", "inputMuted": true})
            )
        ));
        assert!(obs.muted_inputs.contains("Mic/Aux"));
        // Renamed: the mute and the name follow.
        assert!(apply_event(
            &mut obs,
            &event(
                "InputNameChanged",
                json!({"oldInputName": "Mic/Aux", "inputName": "Micro"})
            )
        ));
        assert!(obs.muted_inputs.contains("Micro"));
        assert_eq!(obs.inputs, ["Micro"]);
        // Repeated: no change.
        assert!(!apply_event(
            &mut obs,
            &event("RecordStateChanged", json!({"outputActive": true}))
        ));
        // Unknown events are ignored.
        assert!(!apply_event(
            &mut obs,
            &event("StudioModeStateChanged", json!({}))
        ));
    }

    #[test]
    fn scene_events_keep_the_list_as_in_obs() {
        let mut obs = ObsState::default();
        // GetSceneList lists bottom-up: shown top-down.
        apply_initial(
            &mut obs,
            &json!({ "requestType": "GetSceneList", "responseData": {
                "scenes": [{"sceneName": "Pause"}, {"sceneName": "Jeu"}],
            }}),
        );
        assert_eq!(obs.scenes, ["Jeu", "Pause"]);
        obs.scene = Some("Jeu".into());
        let event = |kind: &str, data: Value| json!({ "eventType": kind, "eventData": data });
        assert!(apply_event(
            &mut obs,
            &event(
                "SceneCreated",
                json!({"sceneName": "Discussion", "isGroup": false})
            )
        ));
        assert_eq!(obs.scenes, ["Jeu", "Pause", "Discussion"]);
        // Renamed: the current scene follows.
        assert!(apply_event(
            &mut obs,
            &event(
                "SceneNameChanged",
                json!({"oldSceneName": "Jeu", "sceneName": "Partie"})
            )
        ));
        assert_eq!(obs.scenes, ["Partie", "Pause", "Discussion"]);
        assert_eq!(obs.scene.as_deref(), Some("Partie"));
        assert!(apply_event(
            &mut obs,
            &event("SceneRemoved", json!({"sceneName": "Pause"}))
        ));
        assert_eq!(obs.scenes, ["Partie", "Discussion"]);
        // Reordered in OBS: the whole list, bottom-up like GetSceneList.
        assert!(apply_event(
            &mut obs,
            &event(
                "SceneListChanged",
                json!({"scenes": [{"sceneName": "Partie"}, {"sceneName": "Discussion"}]})
            )
        ));
        assert_eq!(obs.scenes, ["Discussion", "Partie"]);
    }

    #[test]
    fn a_new_input_list_forgets_the_old_mutes() {
        let mut obs = ObsState {
            inputs: vec!["Mic/Aux".into()],
            muted_inputs: BTreeSet::from(["Mic/Aux".to_owned()]),
            ..ObsState::default()
        };
        let asked = apply_initial(
            &mut obs,
            &json!({ "requestType": "GetInputList", "responseData": {
                "inputs": [{"inputName": "Micro"}, {"inputName": "Bureau"}],
            }}),
        );
        assert_eq!(asked, ["Micro", "Bureau"]);
        assert!(obs.muted_inputs.is_empty());
        // A refused request (no responseData) leaves an empty list.
        apply_initial(
            &mut obs,
            &json!({ "requestType": "GetSceneList", "requestStatus": {"result": false} }),
        );
        assert!(obs.scenes.is_empty());
    }

    #[test]
    fn mute_states_are_asked_in_one_batch() {
        let inputs = vec![
            "Mic/Aux".to_owned(),
            "Desktop Audio".to_owned(),
            "Cam".to_owned(),
        ];
        let batch = mute_request("mutes-7", &inputs);
        assert_eq!(batch["op"], 8);
        assert_eq!(batch["d"]["requestId"], "mutes-7");
        assert_eq!(batch["d"]["executionType"], 0);
        assert_eq!(
            batch["d"]["requests"][1]["requestData"]["inputName"],
            "Desktop Audio"
        );
        let mut obs = ObsState::default();
        let ok = |muted: bool| json!({ "requestStatus": { "result": true, "code": 100 }, "responseData": { "inputMuted": muted } });
        let results =
            json!([ok(true), { "requestStatus": { "result": false, "code": 600 } }, ok(true)]);
        assert_eq!(apply_mutes(&mut obs, &inputs, &results).0, 1);
        assert_eq!(
            obs.muted_inputs,
            BTreeSet::from(["Mic/Aux".to_owned(), "Cam".to_owned()])
        );
        // Missing results count as refused.
        assert_eq!(apply_mutes(&mut obs, &inputs, &json!([ok(false)])).0, 2);
    }

    /// One client connection of [`waits_for_obs_to_finish_starting`]'s OBS:
    /// what it subscribed to and the requests it sent.
    #[derive(Debug, Default)]
    struct Visit {
        subscriptions: u64,
        requests: Vec<String>,
        at: Option<std::time::Instant>,
        /// The close code the client sent, if it closed the connection.
        closed_with: Option<u16>,
    }

    /// A scripted obs-websocket: the first connection is answered 207, the
    /// next ones normally.
    async fn scripted_obs(
        listener: tokio::net::TcpListener,
        visits: std::sync::Arc<std::sync::Mutex<Vec<Visit>>>,
    ) {
        let mut count = 0;
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            count += 1;
            let loading = count == 1;
            let visits = std::sync::Arc::clone(&visits);
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                let index = {
                    let mut visits = visits.lock().unwrap();
                    visits.push(Visit {
                        at: Some(std::time::Instant::now()),
                        ..Visit::default()
                    });
                    visits.len() - 1
                };
                let send = |v: Value| WsMessage::text(v.to_string());
                ws.send(send(
                    json!({ "op": 0, "d": { "obsWebSocketVersion": "5.7.6", "rpcVersion": 1 } }),
                ))
                .await
                .unwrap();
                while let Some(Ok(frame)) = ws.next().await {
                    if let WsMessage::Close(close) = &frame {
                        visits.lock().unwrap()[index].closed_with =
                            Some(close.as_ref().map_or(1005, |f| u16::from(f.code)));
                    }
                    let WsMessage::Text(text) = frame else {
                        continue;
                    };
                    let m: Value = serde_json::from_str(&text).unwrap();
                    let d = &m["d"];
                    let answer = match m["op"].as_u64() {
                        Some(1) => {
                            visits.lock().unwrap()[index].subscriptions =
                                d["eventSubscriptions"].as_u64().unwrap();
                            json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } })
                        }
                        Some(6) => {
                            let request = d["requestType"].as_str().unwrap().to_owned();
                            visits.lock().unwrap()[index].requests.push(request.clone());
                            let (status, data) = if loading {
                                (
                                    json!({ "result": false, "code": 207, "comment": "OBS is not ready to perform the request." }),
                                    json!(null),
                                )
                            } else {
                                let data = match request.as_str() {
                                    "GetInputList" => {
                                        json!({ "inputs": [{ "inputName": "Mic/Aux" }, { "inputName": "Cam" }] })
                                    }
                                    "GetSceneList" => json!({ "scenes": [{ "sceneName": "Jeu" }] }),
                                    "GetCurrentProgramScene" => {
                                        json!({ "currentProgramSceneName": "Jeu" })
                                    }
                                    _ => json!({ "outputActive": false }),
                                };
                                (json!({ "result": true, "code": 100 }), data)
                            };
                            json!({ "op": 7, "d": { "requestType": request, "requestId": d["requestId"], "requestStatus": status, "responseData": data } })
                        }
                        Some(8) => {
                            let requests = d["requests"].as_array().unwrap();
                            visits.lock().unwrap()[index]
                                .requests
                                .push(format!("batch of {}", requests.len()));
                            let results: Vec<Value> = requests.iter().map(|r| json!({
                                "requestType": r["requestType"],
                                "requestStatus": { "result": true, "code": 100 },
                                "responseData": { "inputMuted": r["requestData"]["inputName"] == "Cam" },
                            })).collect();
                            json!({ "op": 9, "d": { "requestId": d["requestId"], "results": results } })
                        }
                        _ => continue,
                    };
                    if ws.send(send(answer)).await.is_err() {
                        return;
                    }
                }
            });
        }
    }

    #[tokio::test]
    async fn waits_for_obs_to_finish_starting() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let visits = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        tokio::spawn(scripted_obs(listener, std::sync::Arc::clone(&visits)));

        let (state, mut observed) = watch::channel(SystemState::default());
        let (_settings, settings_rx) = watch::channel(ObsSettings {
            enabled: true,
            host: "127.0.0.1".into(),
            port,
            password: None,
        });
        let (_requests, requests_rx) = mpsc::channel(4);
        let (status, _status_rx) = watch::channel(ObsStatus::Disabled);
        tokio::spawn(run(settings_rx, requests_rx, state, status));

        let obs = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                observed.changed().await.unwrap();
                if let Observed::Known(obs) = &observed.borrow().obs
                    && !obs.muted_inputs.is_empty()
                {
                    return obs.clone();
                }
            }
        })
        .await
        .expect("OBS state published");
        assert_eq!(obs.scenes, ["Jeu"]);
        assert_eq!(obs.muted_inputs, BTreeSet::from(["Cam".to_owned()]));

        let visits = visits.lock().unwrap();
        assert_eq!(visits.len(), 3, "{visits:?}");
        // Loading: GetVersion answered 207, then nothing more.
        assert_eq!(visits[0].requests, ["GetVersion"]);
        // Loaded: checked once, subscribed to nothing, then left alone.
        assert_eq!(visits[1].subscriptions, 0);
        assert_eq!(visits[1].requests, ["GetVersion"]);
        // Both checks closed normally, not dropped (#15).
        assert_eq!(visits[0].closed_with, Some(1000), "{visits:?}");
        assert_eq!(visits[1].closed_with, Some(1000), "{visits:?}");
        // The real session, STARTUP_SETTLE later: the mutes in one batch.
        assert!(visits[2].at.unwrap() - visits[1].at.unwrap() >= STARTUP_SETTLE);
        assert_eq!(visits[2].subscriptions, u64::from(EVENT_SUBSCRIPTIONS));
        assert_eq!(visits[2].requests[0], "GetVersion");
        assert!(
            visits[2].requests.contains(&"batch of 2".to_owned()),
            "{visits:?}"
        );
        assert!(!visits[2].requests.contains(&"GetInputMute".to_owned()));
    }
}
