//! Padboard desktop app.
//!
//! Wiring: the WebSocket servers (`server`, USB on loopback and Wi-Fi over TLS)
//! run on Tauri's tokio runtime and report [`server::ServerEvent`]s; they are
//! folded into a [`state::Snapshot`] which is pushed to the frontend through
//! the `status` event and can be fetched with the `get_status` command. The
//! other commands drive pairing, revocation and the Wi-Fi toggle. The app
//! lives in the notification area (`tray`): closing the window hides it.

mod adb;
mod appearance;
mod auth;
mod autostart;
mod config;
mod editor_service;
mod identity;
mod locale;
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
mod tablet_update;
mod test_mode;
mod tls;
mod tray;
#[doc(hidden)]
pub mod updates;
mod wifi;

use std::path::{Path, PathBuf};
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
/// Name of the event carrying the updates' [`updates::UpdatesView`].
const UPDATES_EVENT: &str = "updates";

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
    /// Starting with the session: the system's entry, or a flag in test mode
    /// (tests never touch the real autostart).
    autostart: autostart::Autostart,
    updates: updates::Updates,
    /// The tablet APK of this PC's version (downloaded and checked once).
    apks: Arc<updates::tablet::Apks>,
    /// USB tablets being updated (one install at a time per device).
    usb_updating: Mutex<std::collections::HashSet<String>>,
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

    fn set_screensaver(&self, screensaver: preferences::Screensaver) -> Result<(), String> {
        if !preferences::SCREENSAVER_DELAYS.contains(&screensaver.delay_seconds) {
            return Err(format!("delay out of range: {}", screensaver.delay_seconds));
        }
        {
            let mut preferences = self
                .preferences
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            preferences.screensaver = screensaver;
            self.config.save(&*preferences).map_err(|e| e.to_string())?;
        }
        self.server
            .tablet_settings
            .send_replace(screensaver.payload());
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

/// Hides the window in the tray, and remembers the user saw why (first close).
#[tauri::command]
fn hide_to_tray(
    window: tauri::WebviewWindow,
    backend: tauri::State<'_, Backend>,
) -> Result<(), String> {
    {
        let mut preferences = backend
            .preferences
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !preferences.close_hint_shown {
            preferences.close_hint_shown = true;
            backend
                .config
                .save(&*preferences)
                .map_err(|e| e.to_string())?;
        }
    }
    tracing::info!("window hidden in the tray");
    window.hide().map_err(|e| e.to_string())
}

/// Quits the app (first close: "Quit" instead of hiding).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    tracing::info!("quit from the window");
    app.exit(0);
}

/// Whether Padboard starts with the session.
#[tauri::command]
fn get_autostart(backend: tauri::State<'_, Backend>) -> Result<bool, String> {
    backend.autostart.is_enabled()
}

/// Adds or removes the session's autostart entry (it starts hidden in the tray).
#[tauri::command]
fn set_autostart(backend: tauri::State<'_, Backend>, enabled: bool) -> Result<(), String> {
    tracing::info!(enabled, "start with the session");
    backend.autostart.set(enabled)
}

/// The updates card: this version, the choices, what was found.
#[tauri::command]
fn get_updates(backend: tauri::State<'_, Backend>) -> updates::UpdatesView {
    backend.updates.view()
}

/// Saves the update choices (automatic check, test versions).
#[tauri::command]
fn set_update_preferences(
    backend: tauri::State<'_, Backend>,
    preferences: updates::UpdatePreferences,
) -> Result<(), String> {
    {
        let mut saved = backend
            .preferences
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        saved.updates = preferences;
        backend.config.save(&*saved).map_err(|e| e.to_string())?;
    }
    tracing::info!(?preferences, "update preferences");
    backend.updates.set_preferences(preferences);
    Ok(())
}

/// Looks for a new version now.
#[tauri::command]
fn check_for_updates(backend: tauri::State<'_, Backend>) -> Result<(), String> {
    backend.updates.check_now().map_err(|e| e.to_string())
}

/// Downloads, checks and installs the available version (Windows); the app
/// exits once the installer runs, pending edits saved first.
#[tauri::command]
fn install_update(app: tauri::AppHandle, backend: tauri::State<'_, Backend>) -> Result<(), String> {
    let handle = app.clone();
    backend
        .updates
        .install(&app, move || {
            if let Some(backend) = handle.try_state::<Backend>() {
                backend.editor.flush();
            }
            tracing::info!("exiting for the update");
        })
        .map_err(|e| e.to_string())
}

/// Opens the available version's page (or the list of releases) in the
/// browser. The address is built by the backend, never taken from the UI.
#[tauri::command]
async fn open_release_page(backend: tauri::State<'_, Backend>) -> Result<(), String> {
    let target = backend.updates.release_page();
    let action = padboard_actions::Action::Open(padboard_actions::OpenAction { target });
    match server::perform(&backend.server, action, None).await {
        ActionOutcome::Ok => Ok(()),
        outcome => Err(format!("{outcome:?}")),
    }
}

#[tauri::command]
fn get_status(state: tauri::State<'_, SharedSnapshot>) -> Snapshot {
    state.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Shows a new pairing PIN on the desktop (replacing any previous one).
/// The PC's language (`locale.rs`): the editor uses the same as the tablets.
#[tauri::command]
fn get_locale() -> locale::Locale {
    locale::current()
}

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

/// Updates the USB tablet `serial` to this PC's version: the APK of the
/// same release, checked against the signed `SHA256SUMS`, then
/// `adb install -r`. Returns the version installed.
#[tauri::command]
async fn update_usb_tablet(
    backend: tauri::State<'_, Backend>,
    serial: String,
) -> Result<String, adb::UsbUpdateError> {
    if !backend.adb_allowed {
        return Err(adb::UsbUpdateError::TestMode);
    }
    let chosen = backend.adb_path.borrow().clone();
    let adb = adb::locate(chosen.as_deref()).ok_or(adb::UsbUpdateError::NoAdb)?;
    let fresh = backend
        .usb_updating
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(serial.clone());
    if !fresh {
        return Err(adb::UsbUpdateError::Busy);
    }
    tracing::info!(%serial, version = backend.apks.version(), "updating a USB tablet");
    let outcome = async {
        let apk = backend
            .apks
            .get()
            .await
            .map_err(|error| adb::UsbUpdateError::Apk { error })?;
        adb::install_update(&adb::Process, &adb, &serial, &apk.path, apk.version_code).await?;
        Ok(apk.version)
    }
    .await;
    backend
        .usb_updating
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&serial);
    // The device row shows the version read again.
    let _ = backend.adb_refresh.send(());
    outcome
}

/// Sends this PC's version to the connected tablet `client_id` over its
/// connection: kiosk tablets only (they install it silently), checked the
/// same way as for USB. Progress comes with the status (`ClientInfo.update`).
#[tauri::command]
async fn update_wifi_tablet(
    backend: tauri::State<'_, Backend>,
    client_id: u64,
) -> Result<String, tablet_update::WifiUpdateError> {
    use tablet_update::WifiUpdateError;
    let apk = backend
        .apks
        .get()
        .await
        .map_err(|error| WifiUpdateError::Apk { error })?;
    let bytes = tokio::fs::read(&apk.path)
        .await
        .map_err(|e| WifiUpdateError::Apk {
            error: updates::tablet::ApkError::Storage {
                detail: e.to_string(),
            },
        })?;
    let package = tablet_update::UpdatePackage::new(apk.version.clone(), apk.version_code, bytes)
        .ok_or(WifiUpdateError::TooLarge)?;
    tracing::info!(client = client_id, version = %apk.version, "updating a tablet over its connection");
    backend
        .server
        .offer_update(client_id, Arc::new(package))
        .map_err(|reason| WifiUpdateError::Offer { reason })?;
    Ok(apk.version)
}

/// Takes a connected tablet out of kiosk mode for good (`tablet:command`).
#[tauri::command]
fn exit_kiosk(backend: tauri::State<'_, Backend>, client_id: u64) -> bool {
    backend
        .server
        .order(client_id, padboard_protocol::TabletOrder::ExitKiosk)
}

/// The tablets' screensaver settings.
#[tauri::command]
fn get_screensaver(backend: tauri::State<'_, Backend>) -> preferences::Screensaver {
    backend
        .preferences
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .screensaver
}

/// Changes them (persisted, sent to every tablet at once).
#[tauri::command]
fn set_screensaver(
    backend: tauri::State<'_, Backend>,
    screensaver: preferences::Screensaver,
) -> Result<(), String> {
    backend.set_screensaver(screensaver)
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

    tracing::info!(locale = ?locale::current(), "language");
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
            settings.wifi_forced_by_test = true;
        }
    }

    let mut builder = tauri::Builder::default();
    // A second launch (menu, autostart) shows the running app instead of
    // starting another one on the same ports. Not in test mode: a test app
    // must not hand over to the user's own Padboard.
    if test_mode.is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("launched again: showing the window");
            tray::show_window(app);
        }));
    }
    let start_hidden = std::env::args().any(|a| a == tray::HIDDEN_ARG);
    let autostart = if test_mode.is_some() {
        autostart::Autostart::simulated()
    } else {
        autostart::Autostart::system()
    };
    // In-app updates (Windows): the key is the one embedded in the app; the
    // plugin is only driven from Rust (no permission for the page).
    #[cfg(windows)]
    {
        builder = builder.plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(updates::source::Source::current().key_text)
                .build(),
        );
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            hide_to_tray,
            quit_app,
            get_autostart,
            set_autostart,
            get_status,
            get_locale,
            start_pairing,
            cancel_pairing,
            revoke_device,
            set_wifi_enabled,
            set_adb_path,
            enable_kiosk,
            update_usb_tablet,
            update_wifi_tablet,
            exit_kiosk,
            get_screensaver,
            set_screensaver,
            get_editor,
            edit,
            undo,
            redo,
            checkpoint,
            list_keys,
            library,
            test_button,
            get_obs,
            set_obs,
            get_updates,
            set_update_preferences,
            check_for_updates,
            install_update,
            open_release_page
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let config_root = match &test_mode {
                Some(mode) => mode.config_dir(),
                None => app.path().app_config_dir()?,
            };
            let config = ConfigDir::open(config_root)?;
            // Setup runs only in the running instance (a second launch has
            // handed over and exited by now): its log may start afresh.
            if test_mode.is_none() {
                LOG.open(&config.root().join("padboard.log"));
            }
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

            // The tray shows the connected tablets.
            match tray::Tray::create(app.handle(), locale::current()) {
                Ok(tray) => {
                    app.manage(tray);
                }
                Err(e) => tracing::error!("cannot create the tray icon: {e}"),
            }

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
                    if let Some(tray) = handle.try_state::<tray::Tray<tauri::Wry>>() {
                        tray.set_clients(&fresh.clients);
                    }
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
                tablet_settings: tokio::sync::watch::channel(preferences.screensaver.payload()).0,
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

            let updates_handle = app.handle().clone();
            let updates = updates::Updates::new(
                preferences.updates,
                test_mode.is_some(),
                Box::new(move |view| {
                    if let Err(e) = updates_handle.emit(UPDATES_EVENT, view) {
                        tracing::warn!("cannot emit the updates: {e}");
                    }
                }),
            );
            tauri::async_runtime::spawn(updates.clone().run());

            let apks = Arc::new(updates::tablet::Apks::new(config.root().join("updates")));
            autostart.refresh();
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
                autostart,
                updates,
                apks,
                usb_updating: Mutex::default(),
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
                        .permission("core:window:allow-close")
                        .permission("core:window:allow-destroy"),
                )?;
            }

            if let Some(window) = app.get_webview_window("main") {
                window.set_title(if test_mode.is_some() {
                    "Padboard (mode test)"
                } else {
                    "Padboard"
                })?;
                // The window is created hidden: started with the session, it
                // stays in the tray (if there is one to reopen it from).
                if start_hidden && tray::host_available() {
                    tracing::info!("started hidden in the tray");
                } else {
                    window.show()?;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing hides the window in the tray, where the app keeps
            // serving the tablets; explained the first time. Without a tray
            // host (Linux), closing quits: nothing could show the window again.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && tray::host_available()
            {
                api.prevent_close();
                let hint_shown = window.try_state::<Backend>().is_none_or(|backend| {
                    backend
                        .preferences
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .close_hint_shown
                });
                if hint_shown {
                    tracing::info!("window hidden in the tray");
                    let _ = window.hide();
                } else {
                    let _ = window.emit(tray::CLOSE_TO_TRAY_EVENT, ());
                }
            }
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
/// Windows the app has no console, so the file is the only log). The latter
/// is opened by [`DeferredLog::open`] once single instance is settled.
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
        None => None,
    };
    let to_file = file.map(|f| fmt::layer().with_ansi(false).with_writer(Mutex::new(f)));
    let deferred = test_mode
        .is_none()
        .then(|| fmt::layer().with_ansi(false).with_writer(|| &LOG));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(to_file)
        .with(deferred)
        .try_init();
}

/// The normal mode's log file.
static LOG: DeferredLog = DeferredLog(Mutex::new(LogTarget::Pending(Vec::new())));

/// A log file opened late: a second launch reaches the single instance
/// check after logging its first lines, and must not truncate the running
/// instance's log. Lines logged before [`DeferredLog::open`] are kept in
/// memory (bounded) and written first.
struct DeferredLog(Mutex<LogTarget>);

enum LogTarget {
    Pending(Vec<u8>),
    File(std::fs::File),
    Off,
}

/// Lines kept before the file is opened (startup logs only a few).
const PENDING_LOG_MAX: usize = 64 * 1024;

impl DeferredLog {
    /// Creates (truncates) the file and writes the lines logged so far.
    fn open(&self, path: &Path) {
        let mut target = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let LogTarget::Pending(pending) = &*target else {
            return;
        };
        *target = match std::fs::File::create(path) {
            Ok(mut file) => {
                use std::io::Write as _;
                let _ = file.write_all(pending);
                LogTarget::File(file)
            }
            Err(e) => {
                eprintln!("cannot create {}: {e}", path.display());
                LogTarget::Off
            }
        };
    }
}

impl std::io::Write for &DeferredLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut *self.0.lock().unwrap_or_else(PoisonError::into_inner) {
            LogTarget::Pending(pending) => {
                if pending.len() + buf.len() <= PENDING_LOG_MAX {
                    pending.extend_from_slice(buf);
                }
                Ok(buf.len())
            }
            LogTarget::File(file) => std::io::Write::write(file, buf),
            LogTarget::Off => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut *self.0.lock().unwrap_or_else(PoisonError::into_inner) {
            LogTarget::File(file) => std::io::Write::flush(file),
            _ => Ok(()),
        }
    }
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
