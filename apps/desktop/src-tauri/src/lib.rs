//! Padboard desktop app.
//!
//! Wiring: the WebSocket servers (`server`, USB on loopback and Wi-Fi over TLS)
//! run on Tauri's tokio runtime and report [`server::ServerEvent`]s; they are
//! folded into a [`state::Snapshot`] which is pushed to the frontend through
//! the `status` event and can be fetched with the `get_status` command. The
//! other commands drive pairing, revocation and the Wi-Fi toggle.

mod adb;
mod appearance;
mod auth;
mod config;
mod editor_service;
mod identity;
mod netif;
mod obs;
mod observe;
mod paired;
#[doc(hidden)]
pub mod pinning;
mod preferences;
mod server;
mod settings;
mod state;
mod storage;
mod test_mode;
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
use crate::test_mode::TestMode;
use crate::tls::TlsIdentity;
use crate::wifi::Wifi;

/// Name of the event carrying a fresh [`Snapshot`] to the frontend.
const STATUS_EVENT: &str = "status";
/// Name of the event carrying the OBS connection status to the frontend.
const OBS_EVENT: &str = "obs";
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
    /// `adb` may be used (not in simulated test mode).
    adb_allowed: bool,
    /// Asks the USB tracker to check the ready devices again.
    adb_refresh: tokio::sync::mpsc::UnboundedSender<()>,
    /// Devices whose kiosk mode is being turned on (one command at a time).
    kiosk_busy: Mutex<std::collections::HashSet<String>>,
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

/// Kiosk mode for the USB tablet `serial` (Padboard becomes its device owner).
#[tauri::command]
async fn enable_kiosk(
    backend: tauri::State<'_, Backend>,
    serial: String,
) -> Result<(), adb::KioskError> {
    if !backend.adb_allowed {
        return Err(adb::KioskError::TestMode);
    }
    let chosen = backend.adb_path.borrow().clone();
    let adb = adb::locate(chosen.as_deref()).ok_or(adb::KioskError::NoAdb)?;
    let fresh = backend
        .kiosk_busy
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(serial.clone());
    if !fresh {
        return Err(adb::KioskError::Busy);
    }
    let outcome = adb::enable_kiosk(&adb::Process, &adb, &serial).await;
    backend
        .kiosk_busy
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&serial);
    // The device row shows the kiosk state read again, whatever happened.
    let _ = backend.adb_refresh.send(());
    outcome
}

/// Takes a connected tablet out of kiosk mode for good (`tablet:command`).
#[tauri::command]
fn exit_kiosk(backend: tauri::State<'_, Backend>, client_id: u64) -> bool {
    backend
        .server
        .order(client_id, padboard_protocol::TabletOrder::ExitKiosk)
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

/// OBS settings (password left out) and connection status.
#[tauri::command]
fn get_obs(backend: tauri::State<'_, Backend>) -> serde_json::Value {
    let Some(client) = backend.server.obs.get() else {
        return serde_json::json!({ "available": false });
    };
    let settings = client.settings();
    serde_json::json!({
        "available": true,
        "enabled": settings.enabled,
        "host": settings.host,
        "port": settings.port,
        "hasPassword": settings.password.is_some(),
        "status": *client.status().borrow(),
        "scenes": backend.server.state.borrow().obs_names().0,
        "inputs": backend.server.state.borrow().obs_names().1,
    })
}

/// Saves the OBS settings and reconnects. `password: None` keeps the
/// current one, `Some("")` removes it.
#[tauri::command]
fn set_obs(
    backend: tauri::State<'_, Backend>,
    enabled: bool,
    host: String,
    port: u16,
    password: Option<String>,
) -> Result<(), String> {
    let Some(client) = backend.server.obs.get() else {
        return Err("OBS is not available in this mode".into());
    };
    let host = host.trim().to_owned();
    if host.is_empty() || host.len() > 253 || port == 0 {
        return Err("invalid address".into());
    }
    let mut settings = client.settings();
    settings.enabled = enabled;
    settings.host = host;
    settings.port = port;
    match password {
        None => {}
        Some(p) if p.is_empty() => settings.password = None,
        Some(p) => settings.password = Some(p),
    }
    backend.config.save(&settings).map_err(|e| e.to_string())?;
    client.set_settings(settings);
    Ok(())
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
        .flat_map(|p| p.pages.iter().chain(&p.folders))
        .flat_map(|p| &p.buttons)
        .find(|b| b.id == button_id)
        .map(|b| b.action.clone())
        .ok_or_else(|| format!("no button {button_id:?}"))?;
    // No tablet here: navigation steps do nothing.
    Ok(server::perform(&backend.server, action, None).await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let test_mode = TestMode::from_env();
    init_tracing(test_mode.as_ref().ok().and_then(Option::as_ref));

    let mut settings = Settings::from_env();
    for (variable, value) in &settings.invalid {
        tracing::warn!("ignoring invalid {variable}={value:?}, using the default");
    }
    let test_mode = match test_mode {
        Ok(mode) => mode,
        Err(value) => {
            // Never fall back to the real configuration when a test asked for
            // a separate one.
            tracing::error!(
                "{} must be an absolute path, got {value:?}",
                test_mode::VARIABLE
            );
            std::process::exit(2);
        }
    };
    if let Some(mode) = &test_mode {
        if mode.system() {
            tracing::warn!(dir = %mode.config_dir().display(), "TEST MODE (system): separate configuration, actions RUN on this PC, real states, USB and Wi-Fi off");
        } else {
            tracing::warn!(dir = %mode.config_dir().display(), "TEST MODE: separate configuration, actions recorded and never executed, USB and Wi-Fi off");
        }
        // Wi-Fi stays off in tests, except in system mode where
        // PADBOARD_BIND=lan may turn it on (tests on the real tablet).
        if !mode.system() || settings.wifi_forced.is_none() {
            settings.wifi_forced = Some(false);
        }
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
            enable_kiosk,
            exit_kiosk,
            get_editor,
            edit,
            undo,
            redo,
            checkpoint,
            list_keys,
            library,
            test_button,
            get_obs,
            set_obs
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let config_root = match &test_mode {
                Some(mode) => mode.config_dir(),
                None => app.path().app_config_dir()?,
            };
            let config = ConfigDir::open(config_root)?;
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
            let (adb_refresh, adb_refresh_rx) = unbounded_channel::<()>();
            let status_refresh = adb_refresh.clone();
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
                    // A tablet entering or leaving kiosk mode: the USB rows read it again.
                    if matches!(event, ServerEvent::TabletStatus { .. }) {
                        let _ = status_refresh.send(());
                    }
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
            // Button states. In test mode they come from a file, never from
            // the real microphone or player.
            let (state_tx, _) = tokio::sync::watch::channel(appearance::SystemState::default());
            match &test_mode {
                None => observe::start(&state_tx),
                Some(mode) if mode.system() => observe::start(&state_tx),
                Some(mode) => observe::start_from_file(mode.state_file(), &state_tx),
            }
            let ctx = Arc::new(ServerContext {
                server_name,
                server_id: identity.server_id,
                layout: layout_tx,
                state: state_tx,
                executor: OnceLock::new(),
                auth: Arc::clone(&auth),
                events: events_tx.clone(),
                limits: Limits::default(),
                next_connection_id: AtomicU64::new(1),
                handshaking: AtomicUsize::new(0),
                pairing: AtomicUsize::new(0),
                running: Mutex::default(),
                obs: OnceLock::new(),
                sessions: Mutex::default(),
            });
            // OBS: real in normal and system test modes; in simulated test
            // mode its state comes from state.json.
            if test_mode.as_ref().is_none_or(TestMode::system) {
                let settings: obs::ObsSettings = config.load_or_init(obs::ObsSettings::default);
                let client = obs::Obs::start(settings, ctx.state.clone());
                let status_handle = app.handle().clone();
                let mut status = client.status();
                tauri::async_runtime::spawn(async move {
                    loop {
                        let current = status.borrow_and_update().clone();
                        let _ = status_handle.emit(OBS_EVENT, current);
                        if status.changed().await.is_err() {
                            break;
                        }
                    }
                });
                let _ = ctx.obs.set(client);
            }

            // Creating the executor may block (the uinput device needs time to
            // settle), so it is done off the main thread; presses arriving
            // before get an "unavailable" error.
            let executor_ctx = Arc::clone(&ctx);
            let recorder = test_mode
                .as_ref()
                .filter(|mode| !mode.system())
                .map(TestMode::executor);
            tauri::async_runtime::spawn(async move {
                let create = move || match recorder {
                    Some(recorder) => {
                        Box::new(recorder) as Box<dyn padboard_actions::ActionExecutor>
                    }
                    None => padboard_actions::platform_executor(),
                };
                match tokio::task::spawn_blocking(create).await {
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
            // Test mode leaves USB devices alone, except on the real system.
            let adb_allowed = test_mode.as_ref().is_none_or(TestMode::system);
            if adb_allowed {
                tauri::async_runtime::spawn(adb::run(
                    adb::Config::new(settings.usb_port),
                    adb::Process,
                    adb_chosen,
                    adb_refresh_rx,
                    move |status| {
                        let _ = usb_events.send(ServerEvent::Usb(status));
                    },
                ));
            }

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
                adb_allowed,
                adb_refresh,
                kiosk_busy: Mutex::default(),
            });

            // Quitting from a signal (session logout, Ctrl+C in a terminal)
            // goes through RunEvent::Exit, so pending edits are saved.
            let exit_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                termination().await;
                tracing::info!("termination requested, exiting");
                exit_handle.exit(0);
            });

            // The e2e tests close the window from the page, which takes the
            // same path as the user closing it (WebDriver kills the process).
            if test_mode.is_some() {
                app.add_capability(
                    tauri::ipc::CapabilityBuilder::new("e2e-close")
                        .window("main")
                        .permission("core:window:allow-close"),
                )?;
            }

            if let Some(window) = app.get_webview_window("main") {
                window.set_title(if test_mode.is_some() {
                    "Padboard (mode test)"
                } else {
                    "Padboard"
                })?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Tauri application")
        .run(|app, event| {
            // Edits not saved yet (debounce) must not be lost.
            if let tauri::RunEvent::Exit = event {
                if let Some(backend) = app.try_state::<Backend>() {
                    backend.editor.flush();
                }
                tracing::info!("exited");
            }
        });
}

/// Logs to stderr and to a file: `<test dir>/app.log` in test mode, else
/// `padboard.log` in the config directory, started afresh at each launch (on
/// Windows the app has no console, so the file is the only log).
fn init_tracing(test_mode: Option<&TestMode>) {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_env("PADBOARD_LOG").unwrap_or_else(|_| {
        EnvFilter::new("info,padboard_desktop_lib=debug,padboard_actions=debug")
    });
    let file = match test_mode {
        Some(mode) => std::fs::create_dir_all(mode.dir()).ok().and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(mode.log_file())
                .ok()
        }),
        None => default_config_dir().and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::File::create(dir.join("padboard.log")).ok()
        }),
    };
    let to_file = file.map(|f| fmt::layer().with_ansi(false).with_writer(Mutex::new(f)));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(to_file)
        .try_init();
}

/// Tauri's app config directory, known before Tauri starts (for the log).
fn default_config_dir() -> Option<PathBuf> {
    const IDENTIFIER: &str = "io.github.hugoambiehl.padboard";
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join(IDENTIFIER))
}

/// Resolves on SIGTERM, SIGINT or SIGHUP (Ctrl+C / Ctrl+Break on Windows).
async fn termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let kinds = [
            SignalKind::terminate(),
            SignalKind::interrupt(),
            SignalKind::hangup(),
        ];
        let mut signals: Vec<_> = kinds.into_iter().filter_map(|k| signal(k).ok()).collect();
        if signals.is_empty() {
            return std::future::pending().await;
        }
        let waits = signals.iter_mut().map(|s| Box::pin(s.recv()));
        futures_util::future::select_all(waits).await;
    }
    #[cfg(not(unix))]
    {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
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
