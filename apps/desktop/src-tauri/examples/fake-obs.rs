//! A fake OBS Studio, to test the desktop app's OBS client without OBS: a
//! small obs-websocket v5 server (no password) with a scene list and one
//! audio input, answering the requests Padboard sends.
//!
//! ```sh
//! cargo run -p padboard-desktop --example fake-obs -- --port 4455 Jeu Pause
//! ```
//!
//! Scenes are given top-down, as OBS shows them (the first one is the
//! current scene). Once it listens it prints `listening <port>`, then
//! `identified` for each client. Commands on stdin, one per line (scene
//! names without spaces), each sent to the clients as OBS events:
//! `create <scene>`, `rename <old> <new>`, `remove <scene>`.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message as WsMessage;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const INPUT: &str = "Mic/Aux";

#[derive(Debug, Default)]
struct Obs {
    /// Top-down, as OBS shows them.
    scenes: Vec<String>,
    current: Option<String>,
}

type Shared = Arc<Mutex<Obs>>;

#[tokio::main]
async fn main() -> Result<()> {
    let mut port = 4455;
    let mut scenes = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--port" {
            port = args.next().and_then(|p| p.parse().ok()).unwrap_or_else(|| {
                eprintln!("usage: fake-obs [--port N] [scene ...]");
                std::process::exit(2);
            });
        } else {
            scenes.push(arg);
        }
    }
    let obs: Shared = Arc::new(Mutex::new(Obs {
        current: scenes.first().cloned(),
        scenes,
    }));
    let (events, _) = broadcast::channel::<Value>(64);

    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    println!("listening {}", listener.local_addr()?.port());

    let commands_obs = Arc::clone(&obs);
    let commands_events = events.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match command(&commands_obs, &line) {
                Some(event) => {
                    let _ = commands_events.send(event);
                    println!("sent {line}");
                }
                None => eprintln!("fake-obs: unknown command {line:?}"),
            }
        }
    });

    loop {
        let (stream, _) = listener.accept().await?;
        let obs = Arc::clone(&obs);
        let events = events.subscribe();
        tokio::spawn(async move {
            if let Err(e) = client(stream, obs, events).await {
                eprintln!("fake-obs: client: {e}");
            }
        });
    }
}

/// Applies a stdin command; returns the event to send.
fn command(obs: &Shared, line: &str) -> Option<Value> {
    let mut obs = obs.lock().unwrap();
    let words: Vec<&str> = line.split_whitespace().collect();
    let (kind, data) = match words.as_slice() {
        ["create", name] => {
            obs.scenes.push((*name).to_owned());
            (
                "SceneCreated",
                json!({ "sceneName": name, "isGroup": false }),
            )
        }
        ["rename", old, new] => {
            for scene in &mut obs.scenes {
                if scene == old {
                    (*new).clone_into(scene);
                }
            }
            if obs.current.as_deref() == Some(*old) {
                obs.current = Some((*new).to_owned());
            }
            (
                "SceneNameChanged",
                json!({ "oldSceneName": old, "sceneName": new }),
            )
        }
        ["remove", name] => {
            obs.scenes.retain(|s| s != name);
            (
                "SceneRemoved",
                json!({ "sceneName": name, "isGroup": false }),
            )
        }
        _ => return None,
    };
    Some(json!({ "op": 5, "d": { "eventType": kind, "eventIntent": 4, "eventData": data } }))
}

async fn client(
    stream: TcpStream,
    obs: Shared,
    mut events: broadcast::Receiver<Value>,
) -> Result<()> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    let send = |value: Value| WsMessage::text(value.to_string());
    ws.send(send(json!({ "op": 0, "d": {
        "obsWebSocketVersion": "5.7.6-fake",
        "rpcVersion": 1,
    }})))
    .await?;
    let mut identified = false;
    loop {
        tokio::select! {
            frame = ws.next() => {
                let Some(frame) = frame else { return Ok(()) };
                let WsMessage::Text(text) = frame? else { continue };
                let message: Value = serde_json::from_str(&text)?;
                match message["op"].as_u64() {
                    Some(1) => {
                        identified = true;
                        ws.send(send(json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }))).await?;
                        println!("identified");
                    }
                    Some(6) if identified => {
                        let (answer, event) = request(&obs, &message["d"]);
                        ws.send(send(answer)).await?;
                        if let Some(event) = event {
                            ws.send(send(event)).await?;
                        }
                    }
                    _ => {}
                }
            }
            event = events.recv() => {
                let Ok(event) = event else { continue };
                if identified {
                    ws.send(send(event)).await?;
                }
            }
        }
    }
}

/// The answer to a request, and the event it causes.
fn request(obs: &Shared, d: &Value) -> (Value, Option<Value>) {
    let mut obs = obs.lock().unwrap();
    let data = &d["requestData"];
    let mut event = None;
    let response = match d["requestType"].as_str().unwrap_or_default() {
        "GetCurrentProgramScene" => Some(json!({
            "currentProgramSceneName": obs.current,
            "sceneName": obs.current,
        })),
        "GetRecordStatus" | "GetStreamStatus" => Some(json!({ "outputActive": false })),
        // OBS lists scenes bottom-up.
        "GetSceneList" => {
            let scenes: Vec<Value> = obs
                .scenes
                .iter()
                .rev()
                .enumerate()
                .map(|(index, name)| json!({ "sceneName": name, "sceneIndex": index }))
                .collect();
            Some(json!({ "currentProgramSceneName": obs.current, "scenes": scenes }))
        }
        "GetInputList" => Some(json!({ "inputs": [{ "inputName": INPUT }] })),
        "GetInputMute" => Some(json!({ "inputMuted": false })),
        "SetCurrentProgramScene" => {
            let name = data["sceneName"].as_str().unwrap_or_default();
            if obs.scenes.iter().any(|s| s == name) {
                obs.current = Some(name.to_owned());
                event = Some(json!({ "op": 5, "d": {
                    "eventType": "CurrentProgramSceneChanged",
                    "eventIntent": 4,
                    "eventData": { "sceneName": name },
                }}));
                Some(json!({}))
            } else {
                None
            }
        }
        _ => None,
    };
    let status = match &response {
        Some(_) => json!({ "result": true, "code": 100 }),
        None => json!({ "result": false, "code": 600, "comment": "not found" }),
    };
    let mut answer = json!({ "op": 7, "d": {
        "requestType": d["requestType"],
        "requestId": d["requestId"],
        "requestStatus": status,
    }});
    if let Some(response) = response {
        answer["d"]["responseData"] = response;
    }
    (answer, event)
}
