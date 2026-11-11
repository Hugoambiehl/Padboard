//! Test mode, for the end-to-end tests that drive the real window
//! (`apps/desktop/e2e`).
//!
//! Enabled by `PADBOARD_TEST_DIR=<absolute directory>`:
//! - the configuration lives in `<dir>/config` instead of the user's config
//!   directory, so a test never reads or overwrites the real profiles;
//! - actions are appended to `<dir>/actions.jsonl` (one JSON action per line)
//!   and **never** injected into the system (no uinput, no SendInput);
//! - USB devices are left alone (no `adb track-devices` / `adb reverse`), and
//!   Wi-Fi and mDNS stay off whatever the preferences say;
//! - logs go to `<dir>/app.log`;
//! - button states come from `<dir>/state.json` (simulated), never from the
//!   real microphone, sound server or players.
//!
//! With `PADBOARD_TEST_SYSTEM=1`, actions run and states are observed on the
//! real system instead (tests on the real tablet and sound devices); the
//! configuration still lives in the test directory.
//!
//! The window title says the app is in test mode.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use padboard_actions::{Action, ActionError, ActionExecutor};

pub const VARIABLE: &str = "PADBOARD_TEST_DIR";
/// With test mode: run actions and observe states on the real system (the
/// configuration stays in the test directory). For tests on the real
/// tablet and sound devices.
pub const SYSTEM_VARIABLE: &str = "PADBOARD_TEST_SYSTEM";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestMode {
    dir: PathBuf,
    system: bool,
}

impl TestMode {
    /// Reads [`VARIABLE`]. `Err` carries the rejected value: a relative path
    /// could land anywhere, so it is refused rather than guessed.
    pub fn from_env() -> Result<Option<Self>, String> {
        let system = std::env::var(SYSTEM_VARIABLE).is_ok_and(|v| v == "1");
        Ok(
            Self::from_var(std::env::var_os(VARIABLE).map(PathBuf::from))?
                .map(|mode| Self { system, ..mode }),
        )
    }

    /// Actions really run and states really observed (`PADBOARD_TEST_SYSTEM=1`).
    #[must_use]
    pub fn system(&self) -> bool {
        self.system
    }

    fn from_var(value: Option<PathBuf>) -> Result<Option<Self>, String> {
        match value {
            None => Ok(None),
            Some(dir) if dir.as_os_str().is_empty() => Ok(None),
            Some(dir) if dir.is_absolute() => Ok(Some(Self { dir, system: false })),
            Some(dir) => Err(dir.display().to_string()),
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Simulated button states (`observe::start_from_file`).
    #[must_use]
    pub fn state_file(&self) -> PathBuf {
        self.dir.join("state.json")
    }

    #[must_use]
    pub fn log_file(&self) -> PathBuf {
        self.dir.join("app.log")
    }

    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.dir.join("config")
    }

    #[must_use]
    pub fn actions_file(&self) -> PathBuf {
        self.dir.join("actions.jsonl")
    }

    #[must_use]
    pub fn executor(&self) -> RecordingExecutor {
        RecordingExecutor::new(self.actions_file())
    }
}

/// Executor of the test mode: records actions, executes nothing.
#[derive(Debug)]
pub struct RecordingExecutor {
    file: Mutex<PathBuf>,
}

impl RecordingExecutor {
    #[must_use]
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: Mutex::new(file.into()),
        }
    }

    fn append(path: &Path, action: &Action) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(action)?;
        line.push(b'\n');
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(&line)
    }
}

impl ActionExecutor for RecordingExecutor {
    fn execute(&self, action: &Action) -> Result<(), ActionError> {
        // The lock keeps concurrent presses from interleaving their lines.
        let path = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        tracing::info!(?action, "test mode: action recorded, not executed");
        Self::append(&path, action).map_err(|e| ActionError::Failed(format!("cannot record: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use padboard_actions::{Hotkey, Key};

    #[test]
    fn only_an_absolute_directory_enables_it() {
        assert_eq!(TestMode::from_var(None), Ok(None));
        assert_eq!(TestMode::from_var(Some(PathBuf::new())), Ok(None));
        assert!(TestMode::from_var(Some("relative/dir".into())).is_err());
        let dir = std::env::temp_dir().join("padboard-e2e");
        let mode = TestMode::from_var(Some(dir.clone())).unwrap().unwrap();
        assert_eq!(mode.config_dir(), dir.join("config"));
        assert_eq!(mode.actions_file(), dir.join("actions.jsonl"));
    }

    #[test]
    fn actions_are_recorded_one_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let mode = TestMode::from_var(Some(dir.path().to_owned()))
            .unwrap()
            .unwrap();
        let executor = mode.executor();
        let action = Action::Hotkey(Hotkey {
            keys: vec![Key::ControlLeft, Key::KeyC],
        });
        executor.execute(&action).unwrap();
        executor.execute(&action).unwrap();

        let text = std::fs::read_to_string(mode.actions_file()).unwrap();
        let lines: Vec<Action> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines, [action.clone(), action]);
    }
}
