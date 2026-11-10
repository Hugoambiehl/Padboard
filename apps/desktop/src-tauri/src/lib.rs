//! Padboard desktop app.
//!
//! Wiring: the WebSocket server (`server`) runs on Tauri's tokio runtime and
//! reports [`server::ServerEvent`]s; they are folded into a [`state::Snapshot`]
//! which is pushed to the frontend through the `status` event and can be
//! fetched with the `get_status` command.

mod demo;
mod identity;
// Wired into the server and the UI in the next steps of phase 2.
#[allow(dead_code)]
mod paired;
#[allow(dead_code)]
mod preferences;
mod server;
mod settings;
mod state;
mod storage;

use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

use tauri::{Emitter, Manager};
use tokio::sync::mpsc::unbounded_channel;

use crate::identity::Identity;
use crate::server::ServerContext;
use crate::settings::Settings;
use crate::state::Snapshot;
use crate::storage::ConfigDir;

/// Name of the event carrying a fresh [`Snapshot`] to the frontend.
const STATUS_EVENT: &str = "status";

type SharedSnapshot = Arc<Mutex<Snapshot>>;

#[tauri::command]
fn get_status(state: tauri::State<'_, SharedSnapshot>) -> Snapshot {
    state.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    let settings = Settings::from_env();
    for (variable, value) in &settings.invalid {
        tracing::warn!("ignoring invalid {variable}={value:?}, using the default");
    }
    let profile = demo::profile();
    let snapshot: SharedSnapshot = Arc::new(Mutex::new(Snapshot::new(
        &settings,
        lan_addresses(),
        &profile.layout,
    )));

    tauri::Builder::default()
        .manage(Arc::clone(&snapshot))
        .invoke_handler(tauri::generate_handler![get_status])
        .setup(move |app| {
            let handle = app.handle().clone();
            let config = ConfigDir::open(app.path().app_config_dir()?)?;
            tracing::info!(dir = %config.root().display(), "config directory");
            let identity = Identity::load_or_create(&config);
            let (events_tx, mut events_rx) = unbounded_channel();

            // Fold server events into the snapshot and push it to the UI.
            tauri::async_runtime::spawn(async move {
                while let Some(event) = events_rx.recv().await {
                    let fresh = {
                        let mut s = snapshot.lock().unwrap_or_else(PoisonError::into_inner);
                        s.apply(event);
                        s.clone()
                    };
                    if let Err(e) = handle.emit(STATUS_EVENT, fresh) {
                        tracing::warn!("cannot emit status: {e}");
                    }
                }
            });

            // Creating the executor may block (the uinput device needs time to
            // settle), so it is done off the main thread.
            tauri::async_runtime::spawn(async move {
                let executor =
                    match tokio::task::spawn_blocking(padboard_actions::platform_executor).await {
                        Ok(executor) => Arc::from(executor),
                        Err(e) => {
                            tracing::error!("cannot create the action executor: {e}");
                            return;
                        }
                    };
                let ctx = Arc::new(ServerContext {
                    server_name: server_name(),
                    server_id: identity.server_id,
                    profile,
                    executor,
                    events: events_tx,
                });
                server::run(settings.socket_addr(), ctx).await;
            });

            if let Some(window) = app.get_webview_window("main") {
                window.set_title("Padboard")?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Tauri application");
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_env("PADBOARD_LOG").unwrap_or_else(|_| {
        EnvFilter::new("info,padboard_desktop_lib=debug,padboard_actions=debug")
    });
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Hostname, shown in the tablet's top bar.
fn server_name() -> String {
    let name = gethostname::gethostname().to_string_lossy().into_owned();
    if name.is_empty() {
        "Padboard".to_owned()
    } else {
        name
    }
}

/// Non-loopback IPv4 addresses of this PC, sorted.
fn lan_addresses() -> Vec<String> {
    let mut addresses: Vec<String> = match if_addrs::get_if_addrs() {
        Ok(interfaces) => interfaces
            .into_iter()
            .filter(|i| !i.is_loopback())
            .filter_map(|i| match i.ip() {
                IpAddr::V4(ip) if !ip.is_link_local() => Some(ip.to_string()),
                _ => None,
            })
            .collect(),
        Err(e) => {
            tracing::warn!("cannot list network interfaces: {e}");
            Vec::new()
        }
    };
    addresses.sort();
    addresses.dedup();
    addresses
}
