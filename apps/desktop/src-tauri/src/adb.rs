//! USB: follows Android devices through the `adb` server and runs
//! `adb reverse tcp:PORT tcp:PORT` on each device where Padboard is installed,
//! at every (re)plug. The tablet then reaches the USB port at `127.0.0.1`.
//!
//! - `adb` comes from the setting chosen in the UI, else `PATH`, else the
//!   Android SDK (`ANDROID_HOME`, `ANDROID_SDK_ROOT`, default SDK folders).
//!   Downloading Platform-Tools is postponed to phase 6 (license question).
//! - Devices are tracked with the adb server protocol (`host:track-devices` on
//!   `127.0.0.1:5037`): it pushes the device list on every change, without
//!   polling. One-shot commands (`pm path`, `reverse`, `getprop`) run the
//!   `adb` executable.
//! - Devices without Padboard (e.g. a phone plugged for development) are left
//!   alone; they are checked again every few seconds in case Padboard gets
//!   installed.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, timeout};

const PACKAGE: &str = "io.github.hugoambiehl.padboard";
const DEFAULT_SERVER_PORT: u16 = 5037;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const RECHECK_EVERY: Duration = Duration::from_secs(5);
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(10);
/// Bounds the size of a device list message (4 hex digits allow 64 KiB).
const MAX_MESSAGE: usize = 0xffff;

// ─── UI state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbStatus {
    pub adb: AdbState,
    pub devices: Vec<UsbDevice>,
}

impl Default for UsbStatus {
    fn default() -> Self {
        Self {
            adb: AdbState::Searching,
            devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AdbState {
    Searching,
    /// Not found; `chosen` is the path set in the UI if it is the one missing.
    #[serde(rename_all = "camelCase")]
    Missing {
        chosen: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Ready {
        path: String,
        version: Option<String>,
    },
    /// Found but its server cannot be reached; retried automatically.
    #[serde(rename_all = "camelCase")]
    Error {
        path: String,
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDevice {
    pub serial: String,
    pub model: Option<String>,
    pub state: DeviceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DeviceState {
    /// USB debugging not yet allowed on the device.
    Unauthorized,
    /// Linux: the udev rules do not let this user open the device.
    NoPermission,
    Offline,
    /// Looking for Padboard / setting up the redirection.
    Checking,
    /// Padboard is not installed: nothing done.
    NoPadboard,
    /// `adb reverse` is active: the tablet can connect.
    Ready,
    Failed {
        detail: String,
    },
    /// Any other adb state (`recovery`, `bootloader`…).
    Other {
        adb_state: String,
    },
}

// ─── adb executable ──────────────────────────────────────────────────────

fn executable_name() -> &'static str {
    if cfg!(windows) { "adb.exe" } else { "adb" }
}

/// Where to find `adb`: the chosen path, else `PATH`, else the SDK folders.
#[must_use]
pub fn locate(chosen: Option<&Path>) -> Option<PathBuf> {
    if let Some(chosen) = chosen {
        return chosen.is_file().then(|| chosen.to_path_buf());
    }
    let name = executable_name();
    let from_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name));
    let sdk_roots = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .chain(default_sdk_root());
    let from_sdk = sdk_roots.map(|root| root.join("platform-tools").join(name));
    from_path.chain(from_sdk).find(|p| p.is_file())
}

fn default_sdk_root() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Android").join("Sdk"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Android/sdk"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Android/Sdk"))
    }
}

/// Runs one-shot `adb` commands. Abstracted for the tests.
pub trait Runner: Send + Sync + 'static {
    /// `adb <args>`; `Ok(stdout)` on exit code 0, `Err(details)` otherwise.
    fn run(
        &self,
        adb: &Path,
        args: Vec<String>,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

/// The real `adb` executable.
#[derive(Debug, Clone, Copy)]
pub struct Process;

impl Runner for Process {
    async fn run(&self, adb: &Path, args: Vec<String>) -> Result<String, String> {
        let mut command = tokio::process::Command::new(adb);
        command
            .args(&args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        // No console window flashing for each command on Windows.
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let output = match timeout(COMMAND_TIMEOUT, command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => return Err(format!("adb {} timed out", args.join(" "))),
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() {
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("{} {}", stderr.trim(), stdout.trim())
                .trim()
                .to_owned())
        }
    }
}

/// Puts the tablet `serial` in kiosk mode: Padboard becomes its device owner
/// (refused by Android if the tablet has accounts or another owner; the
/// error says why), then is brought to the front, where it locks itself.
///
/// The Android 12L taskbar of LineageOS stays drawn over an app in lock task
/// mode, without reporting any inset: it is turned off too (LineageOS
/// setting, ignored elsewhere; the plain navigation bar replaces it).
///
/// # Errors
/// The output of the failed `adb` command.
pub async fn enable_kiosk<R: Runner>(runner: &R, adb: &Path, serial: &str) -> Result<(), String> {
    let shell = |command: &str| {
        let mut args = vec!["-s".to_owned(), serial.to_owned(), "shell".to_owned()];
        args.extend(command.split(' ').map(str::to_owned));
        args
    };
    let out = runner
        .run(
            adb,
            shell(&format!("dpm set-device-owner {PACKAGE}/.kiosk.KioskAdmin")),
        )
        .await?;
    tracing::info!(serial, output = out.trim(), "kiosk mode enabled");
    let taskbar = runner
        .run(
            adb,
            shell("content insert --uri content://lineagesettings/system --bind name:s:enable_taskbar --bind value:s:0"),
        )
        .await;
    if let Err(e) = taskbar {
        tracing::info!(serial, error = %e, "taskbar setting not applied (not LineageOS?)");
    }
    runner
        .run(adb, shell(&format!("am start -n {PACKAGE}/.MainActivity")))
        .await
        .map(drop)
}

// ─── adb server protocol ─────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("adb server refused the request: {0}")]
    Refused(String),
    #[error("malformed answer from the adb server")]
    Malformed,
}

/// Opens `host:track-devices`; the server then pushes the device list.
async fn track_devices(port: u16) -> Result<TcpStream, ProtocolError> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    let request = "host:track-devices";
    stream
        .write_all(format!("{:04x}{request}", request.len()).as_bytes())
        .await?;
    let mut status = [0u8; 4];
    stream.read_exact(&mut status).await?;
    match &status {
        b"OKAY" => Ok(stream),
        b"FAIL" => Err(ProtocolError::Refused(read_message(&mut stream).await?)),
        _ => Err(ProtocolError::Malformed),
    }
}

/// One length-prefixed message (4 hex digits, then the payload).
async fn read_message(stream: &mut TcpStream) -> Result<String, ProtocolError> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await?;
    let len = std::str::from_utf8(&len)
        .ok()
        .and_then(|l| usize::from_str_radix(l, 16).ok())
        .filter(|l| *l <= MAX_MESSAGE)
        .ok_or(ProtocolError::Malformed)?;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    String::from_utf8(payload).map_err(|_| ProtocolError::Malformed)
}

/// `serial<TAB>state` lines → `(serial, state)`.
fn parse_devices(list: &str) -> Vec<(String, String)> {
    list.lines()
        .filter_map(|line| {
            let (serial, state) = line.split_once('\t')?;
            let serial = serial.trim();
            (!serial.is_empty()).then(|| (serial.to_owned(), state.trim().to_owned()))
        })
        .collect()
}

fn state_of(adb_state: &str) -> DeviceState {
    match adb_state {
        "device" => DeviceState::Checking,
        "unauthorized" | "authorizing" => DeviceState::Unauthorized,
        "offline" | "connecting" => DeviceState::Offline,
        s if s.starts_with("no permissions") => DeviceState::NoPermission,
        other => DeviceState::Other {
            adb_state: other.to_owned(),
        },
    }
}

// ─── Tracker ─────────────────────────────────────────────────────────────

/// What the tracker needs.
#[derive(Debug, Clone)]
pub struct Config {
    /// Port to forward (the USB port of the server).
    pub usb_port: u16,
    /// Port of the adb server (`ANDROID_ADB_SERVER_PORT`, default 5037).
    pub server_port: u16,
}

impl Config {
    #[must_use]
    pub fn new(usb_port: u16) -> Self {
        let server_port = std::env::var("ANDROID_ADB_SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_SERVER_PORT);
        Self {
            usb_port,
            server_port,
        }
    }
}

/// Result of setting a device up.
struct Setup {
    serial: String,
    model: Option<String>,
    state: DeviceState,
}

/// Runs forever: finds `adb`, tracks devices, sets them up. Restarts when the
/// chosen path changes. `report` receives the full status on every change.
pub async fn run<R: Runner + Clone>(
    config: Config,
    runner: R,
    mut chosen: watch::Receiver<Option<PathBuf>>,
    report: impl Fn(UsbStatus) + Send + 'static,
) {
    let mut retry = RETRY_MIN;
    loop {
        let path = chosen.borrow_and_update().clone();
        let outcome = tokio::select! {
            outcome = session(&config, &runner, path.as_deref(), &report) => outcome,
            changed = chosen.changed() => {
                if changed.is_err() {
                    return; // the app is shutting down
                }
                retry = RETRY_MIN;
                continue;
            }
        };
        match outcome {
            // Tracked until the server went away: reconnect quickly.
            Ok(()) => retry = RETRY_MIN,
            Err(()) => retry = (retry * 2).min(RETRY_MAX),
        }
        tokio::select! {
            () = sleep(retry) => {}
            changed = chosen.changed() => {
                if changed.is_err() {
                    return;
                }
                retry = RETRY_MIN;
            }
        }
    }
}

/// One tracking session. `Ok` when the adb server closed the stream after
/// working, `Err` when it could not be used at all.
async fn session<R: Runner + Clone>(
    config: &Config,
    runner: &R,
    chosen: Option<&Path>,
    report: &impl Fn(UsbStatus),
) -> Result<(), ()> {
    let mut status = UsbStatus::default();
    let Some(adb) = locate(chosen) else {
        status.adb = AdbState::Missing {
            chosen: chosen.map(|p| p.display().to_string()),
        };
        report(status);
        return Err(());
    };
    let path = adb.display().to_string();
    // Starts the server if needed (no-op when it runs).
    if let Err(detail) = runner.run(&adb, vec!["start-server".into()]).await {
        tracing::warn!("adb start-server failed: {detail}");
    }
    let stream = match track_devices(config.server_port).await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::warn!("cannot reach the adb server: {e}");
            status.adb = AdbState::Error {
                path,
                detail: e.to_string(),
            };
            report(status);
            return Err(());
        }
    };
    let version = runner
        .run(&adb, vec!["version".into()])
        .await
        .ok()
        .and_then(|out| out.lines().next().map(|l| l.trim().to_owned()));
    status.adb = AdbState::Ready {
        path: path.clone(),
        version,
    };
    tracing::info!(adb = %path, "tracking USB devices");
    report(status.clone());

    // Reading is not cancel-safe (a message may arrive in several parts), so
    // it runs in its own task instead of inside the select! below.
    let (lists_tx, mut lists) = mpsc::unbounded_channel::<String>();
    let reader = tokio::spawn(async move {
        let mut stream = stream;
        while let Ok(list) = read_message(&mut stream).await {
            if lists_tx.send(list).is_err() {
                break;
            }
        }
    });
    let _abort_reader = AbortOnDrop(reader);

    let mut devices: BTreeMap<String, UsbDevice> = BTreeMap::new();
    let (setups_tx, mut setups) = mpsc::unbounded_channel::<Setup>();
    let mut recheck = tokio::time::interval(RECHECK_EVERY);
    recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            message = lists.recv() => {
                let Some(list) = message else {
                    tracing::info!("the adb server closed the device stream");
                    status.devices.clear();
                    status.adb = AdbState::Error { path, detail: "adb server stopped".into() };
                    report(status);
                    return Ok(());
                };
                let seen = parse_devices(&list);
                devices.retain(|serial, _| seen.iter().any(|(s, _)| s == serial));
                for (serial, adb_state) in seen {
                    let state = state_of(&adb_state);
                    let previous = devices.get(&serial).map(|d| d.state.clone());
                    // A ready (or checked) device stays as is while adb says "device".
                    let unchanged = state == DeviceState::Checking
                        && previous.as_ref().is_some_and(|p| {
                            !matches!(p, DeviceState::Unauthorized | DeviceState::Offline | DeviceState::NoPermission | DeviceState::Other { .. })
                        });
                    if unchanged {
                        continue;
                    }
                    if state == DeviceState::Checking {
                        spawn_setup(&adb, runner, config.usb_port, &serial, &setups_tx);
                    }
                    let model = devices.get(&serial).and_then(|d| d.model.clone());
                    devices.insert(serial.clone(), UsbDevice { serial, model, state });
                }
            }
            Some(setup) = setups.recv() => {
                let Some(device) = devices.get_mut(&setup.serial) else { continue };
                if device.state == setup.state && device.model == setup.model {
                    continue;
                }
                match &setup.state {
                    DeviceState::Ready => tracing::info!(serial = %setup.serial, port = config.usb_port, "adb reverse active"),
                    DeviceState::Failed { detail } => tracing::warn!(serial = %setup.serial, "adb setup failed: {detail}"),
                    _ => {}
                }
                device.state = setup.state;
                device.model = setup.model.or(device.model.take());
            }
            _ = recheck.tick() => {
                for device in devices.values().filter(|d| {
                    matches!(d.state, DeviceState::NoPadboard | DeviceState::Failed { .. })
                }) {
                    spawn_setup(&adb, runner, config.usb_port, &device.serial, &setups_tx);
                }
                continue;
            }
        }
        let fresh: Vec<UsbDevice> = devices.values().cloned().collect();
        if fresh != status.devices {
            status.devices = fresh;
            report(status.clone());
        }
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn spawn_setup<R: Runner + Clone>(
    adb: &Path,
    runner: &R,
    usb_port: u16,
    serial: &str,
    results: &mpsc::UnboundedSender<Setup>,
) {
    let adb = adb.to_path_buf();
    let runner = runner.clone();
    let serial = serial.to_owned();
    let results = results.clone();
    tokio::spawn(async move {
        let (model, state) = setup(&adb, &runner, usb_port, &serial).await;
        let _ = results.send(Setup {
            serial,
            model,
            state,
        });
    });
}

/// Checks that Padboard is installed, then forwards the USB port.
async fn setup<R: Runner>(
    adb: &Path,
    runner: &R,
    usb_port: u16,
    serial: &str,
) -> (Option<String>, DeviceState) {
    let on_device = |args: &[&str]| -> Vec<String> {
        ["-s", serial]
            .iter()
            .chain(args)
            .map(|s| (*s).to_owned())
            .collect()
    };
    let model = runner
        .run(adb, on_device(&["shell", "getprop", "ro.product.model"]))
        .await
        .ok()
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty());
    let installed = match runner
        .run(adb, on_device(&["shell", "pm", "path", PACKAGE]))
        .await
    {
        // `pm path` prints "package:/data/app/…" when installed, nothing otherwise
        // (and exits with 1 on recent Android versions).
        Ok(out) => out.contains("package:"),
        Err(_) => false,
    };
    if !installed {
        return (model, DeviceState::NoPadboard);
    }
    let port = format!("tcp:{usb_port}");
    match runner.run(adb, on_device(&["reverse", &port, &port])).await {
        Ok(_) => (model, DeviceState::Ready),
        Err(detail) => (model, DeviceState::Failed { detail }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn parses_device_lists() {
        assert_eq!(parse_devices(""), []);
        assert_eq!(
            parse_devices("R52M1234\tdevice\nemulator-5554\toffline\n"),
            [
                ("R52M1234".to_owned(), "device".to_owned()),
                ("emulator-5554".to_owned(), "offline".to_owned())
            ]
        );
        assert_eq!(parse_devices("garbage\n\tdevice\n"), []);
    }

    #[test]
    fn maps_adb_states() {
        assert_eq!(state_of("device"), DeviceState::Checking);
        assert_eq!(state_of("unauthorized"), DeviceState::Unauthorized);
        assert_eq!(state_of("offline"), DeviceState::Offline);
        assert_eq!(
            state_of("no permissions (missing udev rules? user is in the plugdev group)"),
            DeviceState::NoPermission
        );
        assert_eq!(
            state_of("recovery"),
            DeviceState::Other {
                adb_state: "recovery".into()
            }
        );
    }

    #[test]
    fn a_chosen_path_must_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("adb");
        assert_eq!(locate(Some(&missing)), None);
        std::fs::write(&missing, "").unwrap();
        assert_eq!(locate(Some(&missing)), Some(missing));
    }

    /// Fake `adb` executable: Padboard is installed on the serials listed.
    #[derive(Clone, Default)]
    struct FakeRunner {
        installed: Arc<Mutex<Vec<String>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl Runner for FakeRunner {
        async fn run(&self, _adb: &Path, args: Vec<String>) -> Result<String, String> {
            let line = args.join(" ");
            self.calls.lock().unwrap().push(line.clone());
            match args.as_slice() {
                [s, serial, rest @ ..] if s == "-s" => match rest
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice()
                {
                    ["shell", "getprop", "ro.product.model"] => Ok("SM-T580\n".into()),
                    ["shell", "pm", "path", _] => {
                        if self.installed.lock().unwrap().contains(serial) {
                            Ok("package:/data/app/base.apk\n".into())
                        } else {
                            Err(String::new())
                        }
                    }
                    ["reverse", _, _] => Ok(String::new()),
                    ["shell", "dpm", "set-device-owner", _] => {
                        if serial == "OWNED" {
                            Err("java.lang.IllegalStateException: Not allowed to set the device owner because there are already some accounts on the device".into())
                        } else {
                            Ok("Success: Device owner set\n".into())
                        }
                    }
                    ["shell", "am", "start", "-n", _] => Ok(String::new()),
                    ["shell", "content", "insert", ..] => Ok(String::new()),
                    _ => Err(format!("unexpected {line}")),
                },
                _ => Ok("Android Debug Bridge version 1.0.41\n".into()),
            }
        }
    }

    #[tokio::test]
    async fn kiosk_mode_sets_the_device_owner_then_brings_the_app_up() {
        let runner = FakeRunner::default();
        enable_kiosk(&runner, Path::new("adb"), "TABLET")
            .await
            .unwrap();
        assert_eq!(
            *runner.calls.lock().unwrap(),
            [
                "-s TABLET shell dpm set-device-owner io.github.hugoambiehl.padboard/.kiosk.KioskAdmin",
                "-s TABLET shell content insert --uri content://lineagesettings/system --bind name:s:enable_taskbar --bind value:s:0",
                "-s TABLET shell am start -n io.github.hugoambiehl.padboard/.MainActivity",
            ]
        );
        let refused = enable_kiosk(&runner, Path::new("adb"), "OWNED").await;
        assert!(refused.unwrap_err().contains("accounts"));
        assert_eq!(
            runner.calls.lock().unwrap().len(),
            4,
            "nothing more after a refusal"
        );
    }

    /// Fake adb server: answers `host:track-devices` then pushes the lists
    /// sent on the returned channel.
    async fn fake_server() -> (u16, mpsc::UnboundedSender<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 22];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"0012host:track-devices");
            stream.write_all(b"OKAY").await.unwrap();
            while let Some(list) = rx.recv().await {
                let message = format!("{:04x}{list}", list.len());
                stream.write_all(message.as_bytes()).await.unwrap();
            }
        });
        (port, tx)
    }

    async fn next_status(rx: &mut mpsc::UnboundedReceiver<UsbStatus>) -> UsbStatus {
        timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no status")
            .unwrap()
    }

    /// Waits until a reported status satisfies `pred`.
    async fn wait_for(
        rx: &mut mpsc::UnboundedReceiver<UsbStatus>,
        pred: impl Fn(&UsbStatus) -> bool,
    ) -> UsbStatus {
        loop {
            let status = next_status(rx).await;
            if pred(&status) {
                return status;
            }
        }
    }

    fn device<'a>(status: &'a UsbStatus, serial: &str) -> Option<&'a UsbDevice> {
        status.devices.iter().find(|d| d.serial == serial)
    }

    #[tokio::test]
    async fn sets_up_tablets_with_padboard_and_leaves_other_devices_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let adb = tmp.path().join("adb");
        std::fs::write(&adb, "").unwrap();

        let (server_port, lists) = fake_server().await;
        let runner = FakeRunner::default();
        runner.installed.lock().unwrap().push("TABLET".into());
        let (_chosen_tx, chosen) = watch::channel(Some(adb));
        let (status_tx, mut statuses) = mpsc::unbounded_channel();
        tokio::spawn(run(
            Config {
                usb_port: 47321,
                server_port,
            },
            runner.clone(),
            chosen,
            move |s| {
                let _ = status_tx.send(s);
            },
        ));

        let ready = wait_for(&mut statuses, |s| matches!(s.adb, AdbState::Ready { .. })).await;
        assert!(ready.devices.is_empty());

        lists
            .send("TABLET\tunauthorized\nPHONE\tdevice\n".into())
            .unwrap();
        let status = wait_for(&mut statuses, |s| {
            device(s, "PHONE").is_some_and(|d| d.state == DeviceState::NoPadboard)
        })
        .await;
        assert_eq!(
            device(&status, "TABLET").unwrap().state,
            DeviceState::Unauthorized
        );

        // The user allows USB debugging on the tablet.
        lists
            .send("TABLET\tdevice\nPHONE\tdevice\n".into())
            .unwrap();
        let status = wait_for(&mut statuses, |s| {
            device(s, "TABLET").is_some_and(|d| d.state == DeviceState::Ready)
        })
        .await;
        assert_eq!(
            device(&status, "TABLET").unwrap().model.as_deref(),
            Some("SM-T580")
        );

        let calls = runner.calls.lock().unwrap().clone();
        assert!(calls.contains(&"-s TABLET reverse tcp:47321 tcp:47321".to_owned()));
        assert!(!calls.iter().any(|c| c.starts_with("-s PHONE reverse")));

        // Unplugged.
        lists.send("PHONE\tdevice\n".into()).unwrap();
        wait_for(&mut statuses, |s| device(s, "TABLET").is_none()).await;
    }

    #[tokio::test]
    async fn replugging_sets_the_redirection_up_again() {
        let tmp = tempfile::tempdir().unwrap();
        let adb = tmp.path().join("adb");
        std::fs::write(&adb, "").unwrap();
        let (server_port, lists) = fake_server().await;
        let runner = FakeRunner::default();
        runner.installed.lock().unwrap().push("TABLET".into());
        let (_chosen_tx, chosen) = watch::channel(Some(adb));
        let (status_tx, mut statuses) = mpsc::unbounded_channel();
        tokio::spawn(run(
            Config {
                usb_port: 47321,
                server_port,
            },
            runner.clone(),
            chosen,
            move |s| {
                let _ = status_tx.send(s);
            },
        ));
        let reverses = || {
            runner
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.contains("reverse"))
                .count()
        };
        for round in 1..=2 {
            lists.send("TABLET\tdevice\n".into()).unwrap();
            wait_for(&mut statuses, |s| {
                device(s, "TABLET").is_some_and(|d| d.state == DeviceState::Ready)
            })
            .await;
            assert_eq!(reverses(), round);
            lists.send(String::new()).unwrap();
            wait_for(&mut statuses, |s| s.devices.is_empty()).await;
        }
    }

    #[tokio::test]
    async fn a_missing_adb_is_reported_and_a_new_path_is_picked_up() {
        let tmp = tempfile::tempdir().unwrap();
        let adb = tmp.path().join("adb");
        let (chosen_tx, chosen) = watch::channel(Some(adb.clone()));
        let (status_tx, mut statuses) = mpsc::unbounded_channel();
        let (server_port, _lists) = fake_server().await;
        tokio::spawn(run(
            Config {
                usb_port: 47321,
                server_port,
            },
            FakeRunner::default(),
            chosen,
            move |s| {
                let _ = status_tx.send(s);
            },
        ));
        let status = next_status(&mut statuses).await;
        assert_eq!(
            status.adb,
            AdbState::Missing {
                chosen: Some(adb.display().to_string())
            }
        );
        std::fs::write(&adb, "").unwrap();
        chosen_tx.send(Some(adb)).unwrap();
        wait_for(&mut statuses, |s| matches!(s.adb, AdbState::Ready { .. })).await;
    }

    #[tokio::test]
    async fn an_unreachable_server_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let adb = tmp.path().join("adb");
        std::fs::write(&adb, "").unwrap();
        // A port nobody listens on.
        let port = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (_chosen_tx, chosen) = watch::channel(Some(adb));
        let (status_tx, mut statuses) = mpsc::unbounded_channel();
        tokio::spawn(run(
            Config {
                usb_port: 47321,
                server_port: port,
            },
            FakeRunner::default(),
            chosen,
            move |s| {
                let _ = status_tx.send(s);
            },
        ));
        let status = next_status(&mut statuses).await;
        assert!(matches!(status.adb, AdbState::Error { .. }), "{status:?}");
    }
}
