//! Padboard desktop app.
//!
//! Wiring: the WebSocket servers (`server`, USB on loopback and Wi-Fi over TLS)
//! run on Tauri's tokio runtime and report [`server::ServerEvent`]s; they are
//! folded into a [`state::Snapshot`] which is pushed to the frontend through
//! the `status` event and can be fetched with the `get_status` command. The
//! other commands drive pairing, revocation and the Wi-Fi toggle.

mod adb;
mod auth;
mod config;
mod editor_service;
mod identity;
mod netif;
mod paired;
#[doc(hidden)]
pub mod pinning;
mod preferences;
mod server;
mod settings;
mod state;
mod storage;
mod tls;
mod wifi;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tauri::{Emitter, Manager};
use tokio::sync::mpsc::unbounded_channel;

use crate::auth::{Auth, AuthChange, PairingView, Transport};
use crate::config::EditError;
use crate::config::edit::{Edit, EditOutcome, EditorState};
use crate::editor_service::EditorService;
use crate::identity::Identity;
use crate::preferences::Preferences;
use crate::server::{ActionOutcome, Limits, ServerContext, ServerEvent};
use crate::settings::Settings;
use crate::state::Snapshot;
use crate::storage::ConfigDir;
use crate::tls::TlsIdentity;
use crate::wifi::Wifi;

/// Name of the event carrying a fresh [`Snapshot`] to the frontend.
const STATUS_EVENT: &str = "status";
/// Name of the event carrying a fresh [`EditorState`] to the frontend.
const EDITOR_EVENT: &str = "editor";

type SharedSnapshot = Arc<Mutex<Snapshot>>;

/// Everything the commands act on.
struct Backend {
    config: ConfigDir,
    settings: Settings,
    preferences: Mutex<Preferences>,
    auth: Arc<Auth>,
    wifi: Wifi,
    events: tokio::sync::mpsc::UnboundedSender<ServerEvent>,
    /// The `adb` chosen in the UI; the tracker restarts when it changes.
    adb_path: tokio::sync::watch::Sender<Option<PathBuf>>,
    editor: Arc<EditorService>,
    server: Arc<ServerContext>,
}

impl Backend {
    fn set_wifi_enabled(&self, enabled: bool) -> Result<(), String> {
        if self.settings.wifi_forced.is_some() {
            return Err("Wi-Fi is forced by PADBOARD_BIND".to_owned());
        }
        {
            let mut preferences = self
                .preferences
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            preferences.lan_enabled = enabled;
            self.config.save(&*preferences).map_err(|e| e.to_string())?;
        }
        let _ = self.events.send(ServerEvent::WifiEnabled(enabled));
        self.wifi.set_running(enabled);
        Ok(())
    }

    /// `None`: look `adb` up automatically again.
    fn set_adb_path(&self, path: Option<PathBuf>) -> Result<(), String> {
        if let Some(path) = &path
            && !path.is_file()
        {
            return Err(format!("{} is not a file", path.display()));
        }
        {
            let mut preferences = self
                .preferences
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            preferences.adb_path.clone_from(&path);
            self.config.save(&*preferences).map_err(|e| e.to_string())?;
        }
        self.adb_path.send_replace(path);
        Ok(())
    }
}

#[tauri::command]
fn get_status(state: tauri::State<'_, SharedSnapshot>) -> Snapshot {
    state.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Shows a new pairing PIN on the desktop (replacing any previous one).
#[tauri::command]
fn start_pairing(backend: tauri::State<'_, Backend>) -> Result<PairingView, String> {
    backend.auth.start_pairing().map_err(|e| e.to_string())
}

#[tauri::command]
fn cancel_pairing(backend: tauri::State<'_, Backend>) {
    backend.auth.cancel_pairing();
}

/// Forgets a paired device and closes its Wi-Fi session.
#[tauri::command]
fn revoke_device(backend: tauri::State<'_, Backend>, device_id: String) -> bool {
    backend.auth.revoke(&device_id)
}

/// Turns Wi-Fi connections on or off (persisted). Refused when `PADBOARD_BIND`
/// forces the choice.
#[tauri::command]
fn set_wifi_enabled(backend: tauri::State<'_, Backend>, enabled: bool) -> Result<(), String> {
    backend.set_wifi_enabled(enabled)
}

/// Uses this `adb` executable (`None`: find it automatically).
#[tauri::command]
fn set_adb_path(backend: tauri::State<'_, Backend>, path: Option<String>) -> Result<(), String> {
    backend.set_adb_path(path.map(PathBuf::from))
}

#[tauri::command]
fn get_editor(backend: tauri::State<'_, Backend>) -> EditorState {
    backend.editor.state()
}

/// Applies one edit; the new state follows in an `editor` event.
#[tauri::command]
fn edit(backend: tauri::State<'_, Backend>, edit: Edit) -> Result<EditOutcome, EditError> {
    backend.editor.edit(edit)
}

#[tauri::command]
fn undo(backend: tauri::State<'_, Backend>) -> bool {
    backend.editor.undo()
}

#[tauri::command]
fn redo(backend: tauri::State<'_, Backend>) -> bool {
    backend.editor.redo()
}

/// The user left a text field: its next edit gets its own undo entry.
#[tauri::command]
fn checkpoint(backend: tauri::State<'_, Backend>) {
    backend.editor.checkpoint();
}

/// Valid key names (W3C `KeyboardEvent.code`) for hotkeys.
#[tauri::command]
fn list_keys() -> Vec<&'static str> {
    padboard_actions::Key::ALL
        .iter()
        .map(|k| k.code())
        .collect()
}

/// The action library, adapted to this operating system.
#[tauri::command]
fn library() -> Vec<config::presets::Preset> {
    config::presets::presets()
}

/// Runs a button's action on this PC ("Tester l'action").
#[tauri::command]
async fn test_button(
    backend: tauri::State<'_, Backend>,
    button_id: String,
) -> Result<ActionOutcome, String> {
    let action = backend
        .editor
        .config()
        .profiles
        .iter()
        .flat_map(|p| &p.pages)
        .flat_map(|p| &p.buttons)
        .find(|b| b.id == button_id)
        .map(|b| b.action.clone())
        .ok_or_else(|| format!("no button {button_id:?}"))?;
    Ok(server::execute(&backend.server, action).await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    let settings = Settings::from_env();
    for (variable, value) in &settings.invalid {
        tracing::warn!("ignoring invalid {variable}={value:?}, using the default");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            start_pairing,
            cancel_pairing,
            revoke_device,
            set_wifi_enabled,
            set_adb_path,
            get_editor,
            edit,
            undo,
            redo,
            checkpoint,
            list_keys,
            library,
            test_button
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let config = ConfigDir::open(app.path().app_config_dir()?)?;
            tracing::info!(dir = %config.root().display(), "config directory");
            let identity = Identity::load_or_create(&config);
            let preferences: Preferences = config.load_or_init(Preferences::default);
            let wifi_enabled = settings.wifi_forced.unwrap_or(preferences.lan_enabled);
            let (tls, tls_error) = match TlsIdentity::load_or_create(&config) {
                Ok(tls) => (Some(tls), None),
                Err(e) => {
                    tracing::error!("Wi-Fi unavailable: {e}");
                    (None, Some(e.to_string()))
                }
            };
            let profiles = config::load(&config);
            let profile = profiles.active_layout();
            let server_name = server_name();
            let interfaces = netif::scan();

            let snapshot: SharedSnapshot = Arc::new(Mutex::new(Snapshot::new(state::Init {
                settings: &settings,
                server_name: server_name.clone(),
                lan_addresses: interfaces.addresses.clone(),
                layout: &profile.layout,
                wifi_enabled,
                fingerprint: tls.as_ref().map(|t| t.fingerprint.clone()),
                tls_error,
            })));
            app.manage(Arc::clone(&snapshot));

            let (events_tx, mut events_rx) = unbounded_channel();
            let auth_events = events_tx.clone();
            let auth = Arc::new(Auth::new(
                Some(config.clone()),
                Box::new(move |change| {
                    let _ = auth_events.send(ServerEvent::Auth(change));
                }),
            ));
            let _ = events_tx.send(ServerEvent::Auth(AuthChange::PairedDevices(
                auth.paired_devices(),
            )));

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

            let (layout_tx, _) = tokio::sync::watch::channel(Arc::new(profile));
            let editor_handle = app.handle().clone();
            let editor = EditorService::start(
                profiles,
                config.clone(),
                layout_tx.clone(),
                events_tx.clone(),
                Box::new(move |state| {
                    if let Err(e) = editor_handle.emit(EDITOR_EVENT, state) {
                        tracing::warn!("cannot emit the editor state: {e}");
                    }
                }),
            );
            let ctx = Arc::new(ServerContext {
                server_name,
                server_id: identity.server_id,
                layout: layout_tx,
                executor: OnceLock::new(),
                auth: Arc::clone(&auth),
                events: events_tx.clone(),
                limits: Limits::default(),
                next_connection_id: AtomicU64::new(1),
                handshaking: AtomicUsize::new(0),
                pairing: AtomicUsize::new(0),
            });

            // Creating the executor may block (the uinput device needs time to
            // settle), so it is done off the main thread; presses arriving
            // before get an "unavailable" error.
            let executor_ctx = Arc::clone(&ctx);
            tauri::async_runtime::spawn(async move {
                match tokio::task::spawn_blocking(padboard_actions::platform_executor).await {
                    Ok(executor) => {
                        let _ = executor_ctx.executor.set(Arc::from(executor));
                    }
                    Err(e) => tracing::error!("cannot create the action executor: {e}"),
                }
            });

            let usb_ctx = Arc::clone(&ctx);
            let usb_addr = settings.usb_addr();
            tauri::async_runtime::spawn(async move {
                server::run(usb_addr, Transport::Usb, None, usb_ctx).await;
            });

            let wifi = Wifi::new(
                Arc::clone(&ctx),
                settings.wifi_addr(),
                tls.map(|t| t.acceptor),
            );
            wifi.exclude_interfaces(&interfaces.virtual_names);
            wifi.set_running(wifi_enabled);

            // USB: follow devices and forward the USB port to the tablets.
            let (adb_path, adb_chosen) = tokio::sync::watch::channel(preferences.adb_path.clone());
            let usb_events = events_tx.clone();
            tauri::async_runtime::spawn(adb::run(
                adb::Config::new(settings.usb_port),
                adb::Process,
                adb_chosen,
                move |status| {
                    let _ = usb_events.send(ServerEvent::Usb(status));
                },
            ));

            // Interfaces change (Wi-Fi reconnects, Docker starts): refresh the
            // addresses shown and keep new virtual interfaces out of mDNS.
            let scan_events = events_tx.clone();
            let scan_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut last = interfaces;
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(10));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    let fresh = tauri::async_runtime::spawn_blocking(netif::scan)
                        .await
                        .unwrap_or_default();
                    if fresh == last {
                        continue;
                    }
                    if let Some(backend) = scan_handle.try_state::<Backend>() {
                        backend.wifi.exclude_interfaces(&fresh.virtual_names);
                    }
                    let _ = scan_events.send(ServerEvent::LanAddresses(fresh.addresses.clone()));
                    last = fresh;
                }
            });

            app.manage(Backend {
                config,
                settings: settings.clone(),
                preferences: Mutex::new(preferences),
                auth,
                wifi,
                events: events_tx,
                adb_path,
                editor,
                server: Arc::clone(&ctx),
            });

            if let Some(window) = app.get_webview_window("main") {
                window.set_title("Padboard")?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Tauri application")
        .run(|app, event| {
            // Edits not saved yet (debounce) must not be lost.
            if let tauri::RunEvent::Exit = event
                && let Some(backend) = app.try_state::<Backend>()
            {
                backend.editor.flush();
            }
        });
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
