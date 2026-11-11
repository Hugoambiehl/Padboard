//! OBS Studio through obs-websocket v5 (built into OBS 28+).
//!
//! A small client of our own (the protocol is JSON over WebSocket: `Hello`,
//! `Identify` with a SHA-256 challenge, requests and events) on the
//! WebSocket stack the app already has. One task owns the connection: it
//! connects when OBS is enabled in the settings, retries every few seconds
//! while OBS is closed, publishes what the buttons show (current scene,
//! recording, streaming, muted inputs) and runs the requests of OBS buttons.
//! While it is not connected, OBS buttons are unavailable (dimmed).

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures_util::{SinkExt, StreamExt};
use padboard_actions::{ObsAction, ObsCommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::appearance::{Observed, SystemState};
use crate::storage::Document;

/// Default port of obs-websocket.
pub const DEFAULT_PORT: u16 = 4455;
const RETRY: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(3);
/// Scenes, inputs and outputs events.
const EVENT_SUBSCRIPTIONS: u32 = 4 | 8 | 64;

/// `obs.json` (owner-only: it holds the obs-websocket password).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
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
    loop {
        let current = settings.borrow_and_update().clone();
        if !current.enabled {
            status.send_replace(ObsStatus::Disabled);
            publish(&state, Observed::Unavailable);
            tokio::select! {
                changed = settings.changed() => if changed.is_err() { return },
                Some((_, _, reply)) = requests.recv() => { let _ = reply.send(Err(ObsError::NotConnected)); }
            }
            continue;
        }
        status.send_replace(ObsStatus::Connecting);
        let outcome = session(&current, &mut settings, &mut requests, &state, &status).await;
        publish(&state, Observed::Unavailable);
        match outcome {
            Ok(()) => continue, // settings changed: reconnect now
            Err(ObsError::Auth) => {
                status.send_replace(ObsStatus::AuthFailed);
                // Retrying would fail the same way: wait for new settings.
                loop {
                    tokio::select! {
                        changed = settings.changed() => { if changed.is_err() { return } break }
                        Some((_, _, reply)) = requests.recv() => { let _ = reply.send(Err(ObsError::NotConnected)); }
                    }
                }
            }
            Err(e) => {
                status.send_replace(ObsStatus::Unreachable {
                    detail: e.to_string(),
                });
                let retry = tokio::time::sleep(RETRY);
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
    }
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

async fn send_json(ws: &mut Ws, value: &Value) -> Result<(), ObsError> {
    ws.send(WsMessage::text(value.to_string()))
        .await
        .map_err(|e| ObsError::Connection(e.to_string()))
}

/// Connects, identifies, follows OBS until the connection ends (`Err`) or
/// the settings change (`Ok`).
async fn session(
    settings: &ObsSettings,
    settings_rx: &mut watch::Receiver<ObsSettings>,
    requests: &mut mpsc::Receiver<(String, Value, Reply)>,
    state: &watch::Sender<SystemState>,
    status: &watch::Sender<ObsStatus>,
) -> Result<(), ObsError> {
    let url = format!("ws://{}:{}", settings.host, settings.port);
    let (mut ws, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::connect_async(&url))
        .await
        .map_err(|_| ObsError::Connection(format!("no answer from {url}")))?
        .map_err(|e| ObsError::Connection(format!("{url}: {e}")))?;

    // Hello → Identify → Identified.
    let hello = tokio::time::timeout(TIMEOUT, recv_json(&mut ws))
        .await
        .map_err(|_| ObsError::Connection("no Hello from OBS".into()))??;
    let version = hello["d"]["obsWebSocketVersion"]
        .as_str()
        .unwrap_or("?")
        .to_owned();
    let mut identify = json!({ "rpcVersion": 1, "eventSubscriptions": EVENT_SUBSCRIPTIONS });
    if let Some(auth) = hello["d"].get("authentication") {
        let password = settings.password.as_deref().ok_or(ObsError::Auth)?;
        identify["authentication"] = json!(authentication(
            password,
            auth["salt"].as_str().unwrap_or_default(),
            auth["challenge"].as_str().unwrap_or_default(),
        ));
    }
    send_json(&mut ws, &json!({ "op": 1, "d": identify })).await?;
    let identified = tokio::time::timeout(TIMEOUT, recv_json(&mut ws))
        .await
        .map_err(|_| ObsError::Connection("OBS did not identify us".into()))??;
    if identified["op"] != 2 {
        return Err(ObsError::Connection(format!(
            "unexpected answer: {identified}"
        )));
    }
    tracing::info!(%version, "connected to OBS");
    status.send_replace(ObsStatus::Connected { version });

    let mut next_id: u64 = 0;
    let mut pending: HashMap<String, Reply> = HashMap::new();
    // GetInputMute answers do not name the input: id → input.
    let mut mute_queries: HashMap<String, String> = HashMap::new();
    let mut obs = ObsState::default();

    // Initial state: answers are collected below like any reply.
    let mut ask = |ws_requests: &mut Vec<Value>, request: &str, data: Value| {
        next_id += 1;
        let id = format!("init-{request}-{next_id}");
        ws_requests.push(json!({ "op": 6, "d": { "requestType": request, "requestId": id, "requestData": data } }));
    };
    let mut initial = Vec::new();
    for request in [
        "GetCurrentProgramScene",
        "GetRecordStatus",
        "GetStreamStatus",
        "GetSceneList",
        "GetInputList",
    ] {
        ask(&mut initial, request, json!({}));
    }
    for request in &initial {
        send_json(&mut ws, request).await?;
    }
    // Answers can come in any order: the state is published once all the
    // initial ones are in (no half-filled state on the buttons).
    let mut initial_left = initial.len();
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
                        if let Some(input) = mute_queries.remove(&id) {
                            if d["responseData"]["inputMuted"].as_bool().unwrap_or(false) {
                                obs.muted_inputs.insert(input);
                            }
                            if ready {
                                publish(state, Observed::Known(obs.clone()));
                            }
                        } else if let Some(reply) = pending.remove(&id) {
                            let ok = d["requestStatus"]["result"].as_bool().unwrap_or(false);
                            let _ = reply.send(if ok {
                                Ok(d["responseData"].clone())
                            } else {
                                Err(ObsError::Refused(
                                    d["requestStatus"]["comment"].as_str().unwrap_or("refused").to_owned(),
                                ))
                            });
                        } else if id.starts_with("init-") {
                            let followups = apply_initial(&mut obs, d);
                            for input in followups {
                                next_id += 1;
                                let id = format!("mute-{next_id}");
                                send_json(&mut ws, &json!({ "op": 6, "d": {
                                    "requestType": "GetInputMute",
                                    "requestId": id,
                                    "requestData": { "inputName": input },
                                }})).await?;
                                mute_queries.insert(id, input);
                            }
                            initial_left = initial_left.saturating_sub(1);
                            ready = initial_left == 0;
                            if ready {
                                publish(state, Observed::Known(obs.clone()));
                            }
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
                let _ = ws.close(None).await;
                return if changed.is_ok() { Ok(()) } else { Err(ObsError::NotConnected) };
            }
        }
    }
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
}
