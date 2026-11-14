//! A fake tablet, to test the desktop app without the real one: it speaks the
//! protocol exactly like the Android app (mDNS discovery, TLS with the PC's
//! key pinned, PIN pairing, token, USB without pairing).
//!
//! ```sh
//! cargo run -p padboard-desktop --example fake-tablet -- discover
//! cargo run -p padboard-desktop --example fake-tablet -- wifi 192.168.1.20
//! cargo run -p padboard-desktop --example fake-tablet -- usb
//! cargo run -p padboard-desktop --example fake-tablet -- forget
//! ```
//!
//! Options: `--id NAME` simulates another tablet (its own device id and token),
//! `--port N` overrides the port. Once connected, type a button id (e.g.
//! `terminal`) and Enter to press it; Ctrl+C to quit.
//!
//! The pairing (server id, token, pinned key) is kept in the system temp
//! directory, one file per `--id`.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use padboard_desktop_lib::pinning::{Pin, certificate_key, client_config};
use padboard_protocol::{
    AuthStatus, ButtonEvent, ErrorCode, HelloPayload, MDNS_SERVICE_TYPE, Message, PROTOCOL_VERSION,
    PairRequestPayload, USB_PORT, WIFI_PORT, txt,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, Lines, Stdin};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Input = Lines<BufReader<Stdin>>;

fn input() -> Input {
    BufReader::new(tokio::io::stdin()).lines()
}

#[derive(Default, Serialize, Deserialize)]
struct Pairing {
    device_id: String,
    server_id: Option<String>,
    token: Option<String>,
    /// SHA-256 of the PC's public key, hex.
    spki_sha256: Option<String>,
}

struct Args {
    command: String,
    target: Option<String>,
    id: String,
    port: Option<u16>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut parsed = Args {
        command: String::new(),
        target: None,
        id: "fake-tablet".into(),
        port: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--id" => parsed.id = args.next().unwrap_or_default(),
            "--port" => parsed.port = args.next().and_then(|p| p.parse().ok()),
            _ if parsed.command.is_empty() => parsed.command = arg,
            _ => parsed.target = Some(arg),
        }
    }
    parsed
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    match args.command.as_str() {
        "discover" => discover(),
        "usb" => usb(&args).await,
        "wifi" => wifi(&args).await,
        "forget" => {
            let _ = std::fs::remove_file(state_file(&args.id));
            println!("Forgot the pairing of {:?}.", args.id);
            Ok(())
        }
        _ => {
            eprintln!(
                "usage: fake-tablet discover | usb | wifi <host> | forget  [--id NAME] [--port N]"
            );
            std::process::exit(2);
        }
    }
}

fn state_file(id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join(format!("padboard-fake-tablet-{safe}.json"))
}

fn load(id: &str) -> Pairing {
    std::fs::read_to_string(state_file(id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| Pairing {
            device_id: format!("{id}-{}", uuid::Uuid::new_v4()),
            ..Pairing::default()
        })
}

fn save(id: &str, pairing: &Pairing) {
    let _ = std::fs::write(
        state_file(id),
        serde_json::to_string_pretty(pairing).unwrap(),
    );
}

fn discover() -> Result<()> {
    let daemon = mdns_sd::ServiceDaemon::new()?;
    let events = daemon.browse(MDNS_SERVICE_TYPE)?;
    println!("Looking for Padboard PCs for 4 seconds…");
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    let mut found = 0;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        if let mdns_sd::ServiceEvent::ServiceResolved(service) = event {
            found += 1;
            let addresses: Vec<String> = service.addresses.iter().map(|a| a.to_string()).collect();
            println!(
                "- {} (id {}, protocol {}) at {} port {}",
                service
                    .txt_properties
                    .get_property_val_str(txt::SERVER_NAME)
                    .unwrap_or("?"),
                service
                    .txt_properties
                    .get_property_val_str(txt::SERVER_ID)
                    .unwrap_or("?"),
                service
                    .txt_properties
                    .get_property_val_str(txt::PROTOCOL_VERSION)
                    .unwrap_or("?"),
                addresses.join(", "),
                service.port
            );
        }
    }
    if found == 0 {
        println!("No PC found (is Wi-Fi enabled in Padboard?).");
    }
    let _ = daemon.shutdown();
    Ok(())
}

fn hello(id: &str, pairing: &Pairing) -> Message {
    Message::Hello(HelloPayload {
        protocol_version: PROTOCOL_VERSION,
        device_name: format!("Fake tablet ({id})"),
        device_id: pairing.device_id.clone(),
        token: pairing.token.clone(),
        app_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        app_version_code: None,
        silent_install: None,
    })
}

async fn usb(args: &Args) -> Result<()> {
    let port = args.port.unwrap_or(USB_PORT);
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}")).await?;
    let pairing = load(&args.id);
    save(&args.id, &pairing);
    send(&mut ws, &hello(&args.id, &pairing)).await?;
    let Message::Welcome(welcome) = recv(&mut ws).await? else {
        return Err("expected welcome".into());
    };
    println!(
        "Connected over USB to {} ({:?}).",
        welcome.server_name, welcome.auth
    );
    session(ws, &mut input()).await
}

async fn wifi(args: &Args) -> Result<()> {
    let host = args.target.clone().ok_or("missing <host>")?;
    let port = args.port.unwrap_or(WIFI_PORT);
    let mut pairing = load(&args.id);
    let mut lines = input();

    let pin = match pairing.spki_sha256.as_deref().and_then(parse_hex) {
        Some(spki) => Pin::Spki(spki),
        None => Pin::TrustOnFirstUse,
    };
    let tcp = TcpStream::connect((host.as_str(), port)).await?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config(pin)));
    let name = rustls::pki_types::ServerName::try_from("padboard.local")?;
    let tls = match connector.connect(name, tcp).await {
        Ok(tls) => tls,
        Err(e) => {
            eprintln!("TLS refused: {e}");
            eprintln!("If the PC got a new key, run `fake-tablet forget` and pair again.");
            return Err(e.into());
        }
    };
    let certificate = tls
        .get_ref()
        .1
        .peer_certificates()
        .ok_or("no certificate")?[0]
        .clone();
    let (spki, code) = certificate_key(&certificate).ok_or("unreadable certificate")?;
    let (mut ws, _) = tokio_tungstenite::client_async(format!("wss://{host}:{port}"), tls).await?;

    send(&mut ws, &hello(&args.id, &pairing)).await?;
    let Message::Welcome(welcome) = recv(&mut ws).await? else {
        return Err("expected welcome".into());
    };
    if pairing
        .server_id
        .as_ref()
        .is_some_and(|id| *id != welcome.server_id)
    {
        println!("Another PC than the paired one: its token is not sent.");
    }
    match welcome.auth {
        AuthStatus::Authenticated => {
            println!(
                "Connected over Wi-Fi to {} with the token.",
                welcome.server_name
            );
        }
        status => {
            if status == AuthStatus::TokenRejected {
                println!("The PC rejected the token (revoked?): pairing again.");
            }
            pairing.token = None;
            println!("Pairing with {}.", welcome.server_name);
            println!("Key fingerprint of the PC: {code}  <- must match the one shown on the PC");
            loop {
                let pin = prompt(&mut lines, "PIN shown on the PC (empty to quit): ").await?;
                if pin.is_empty() {
                    return Ok(());
                }
                send(&mut ws, &Message::PairRequest(PairRequestPayload { pin })).await?;
                match recv(&mut ws).await? {
                    Message::PairSuccess(success) => {
                        pairing.token = Some(success.token);
                        pairing.server_id = Some(welcome.server_id.clone());
                        pairing.spki_sha256 = Some(hex(&spki));
                        save(&args.id, &pairing);
                        println!("Paired. The key is pinned and the token saved.");
                        break;
                    }
                    Message::PairFailure(failure) => println!(
                        "Refused: {:?} ({} attempts left)",
                        failure.reason, failure.attempts_left
                    ),
                    other => return Err(format!("unexpected {}", other.type_tag()).into()),
                }
            }
        }
    }
    let result = session(ws, &mut lines).await;
    if let Err(e) = &result
        && e.to_string().contains("revoked")
    {
        pairing.token = None;
        save(&args.id, &pairing);
    }
    result
}

/// Prints what the PC sends; each line typed on stdin presses that button.
async fn session<S: AsyncRead + AsyncWrite + Unpin>(
    ws: WebSocketStream<S>,
    lines: &mut Input,
) -> Result<()> {
    let (mut sink, mut stream) = ws.split();
    println!("Type a button id and Enter to press it (Ctrl+C to quit).");
    // Like the tablet: a ping every 5 s, or the PC closes a silent session.
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    let mut seq = 0u32;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                seq = seq.wrapping_add(1);
                sink.send(WsMessage::text(Message::Ping(padboard_protocol::Heartbeat { seq }).to_json()?)).await?;
            }
            frame = stream.next() => {
                let Some(frame) = frame else {
                    println!("Connection closed by the PC.");
                    return Ok(());
                };
                match frame? {
                    WsMessage::Text(text) => match Message::from_json(&text)? {
                        Message::LayoutSync(layout) => {
                            let ids: Vec<&str> = layout.pages.iter().flat_map(|p| &p.buttons).map(|b| b.id.as_str()).collect();
                            println!("Layout \"{}\": {}", layout.profile.name, ids.join(", "));
                            for b in layout.pages.iter().flat_map(|p| &p.buttons).chain(layout.folders.iter().flat_map(|f| &f.buttons)) {
                                if b.active || b.unavailable {
                                    println!("State {}: active={} unavailable={}", b.id, b.active, b.unavailable);
                                }
                            }
                        }
                        Message::Error(e) => {
                            println!("Error from the PC: {:?} ({})", e.code, e.message);
                            match e.code {
                                ErrorCode::Revoked => return Err("pairing revoked by the PC".into()),
                                ErrorCode::Superseded | ErrorCode::Unauthorized => return Ok(()),
                                _ => {}
                            }
                        }
                        Message::ButtonUpdate(b) => {
                            let mut flags = Vec::new();
                            if b.active { flags.push("active".to_owned()); }
                            if b.unavailable { flags.push("unavailable".to_owned()); }
                            if let Some(badge) = &b.badge { flags.push(format!("badge={badge}")); }
                            if let Some(subtitle) = &b.subtitle { flags.push(format!("({subtitle})")); }
                            println!("Update {}: \"{}\" {}", b.id, b.label, flags.join(" "));
                        }
                        Message::Navigate(n) => {
                            println!("Navigate {:?} {}", n.to, n.id.unwrap_or_default());
                        }
                        Message::Pong(_) => {}
                        other => println!("<- {}", other.type_tag()),
                    },
                    WsMessage::Close(_) => {
                        println!("Connection closed by the PC.");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            line = lines.next_line() => {
                let Some(line) = line? else { return Ok(()) };
                let button_id = line.trim().to_owned();
                if button_id.is_empty() {
                    continue;
                }
                let event = ButtonEvent { button_id };
                sink.send(WsMessage::text(Message::ButtonPress(event.clone()).to_json()?)).await?;
                sink.send(WsMessage::text(Message::ButtonRelease(event).to_json()?)).await?;
            }
        }
    }
}

async fn send<S: AsyncRead + AsyncWrite + Unpin>(
    ws: &mut WebSocketStream<S>,
    message: &Message,
) -> Result<()> {
    ws.send(WsMessage::text(message.to_json()?)).await?;
    Ok(())
}

async fn recv<S: AsyncRead + AsyncWrite + Unpin>(ws: &mut WebSocketStream<S>) -> Result<Message> {
    loop {
        match ws.next().await.ok_or("connection closed")?? {
            WsMessage::Text(text) => {
                let message = Message::from_json(&text)?;
                if let Message::Error(e) = &message {
                    return Err(format!("{:?}: {}", e.code, e.message).into());
                }
                return Ok(message);
            }
            WsMessage::Close(_) => return Err("connection closed".into()),
            _ => {}
        }
    }
}

async fn prompt(lines: &mut Input, question: &str) -> Result<String> {
    print!("{question}");
    std::io::stdout().flush()?;
    Ok(lines
        .next_line()
        .await?
        .unwrap_or_default()
        .trim()
        .to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_hex(s: &str) -> Option<[u8; 32]> {
    let bytes: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| s.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
        .collect();
    bytes?.try_into().ok()
}
