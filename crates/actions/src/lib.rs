//! Action execution: the [`ActionExecutor`] trait and its platform implementations.
//!
//! The implementation is selected at compile time:
//! - Linux: keyboard injection through a uinput virtual device (works on X11 and Wayland);
//! - Windows: `SendInput`;
//! - other platforms: every action returns [`ActionError::Unsupported`].
//!
//! Executors are **blocking** (a hotkey holds its keys for a few milliseconds):
//! async callers must run them on a blocking thread (`tokio::task::spawn_blocking`).
//! Executors log failures themselves, and never panic.

use std::collections::HashSet;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub mod audio;
mod keys;
mod kinds;
pub mod state;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(windows)]
mod win;

pub use keys::{Key, WinKey};
pub use kinds::*;

/// Maximum number of keys in a single hotkey.
pub const MAX_HOTKEY_KEYS: usize = 6;

/// How long the keys of a hotkey stay pressed. Some applications (games
/// especially) ignore key presses shorter than a frame.
const HOTKEY_HOLD: Duration = Duration::from_millis(20);

/// Something a button can do. Stored in the desktop configuration, never sent
/// to the tablet. JSON: `{ "type": "hotkey", "params": { "keys": [...] } }`
/// (adjacently tagged, the form typeshare can generate for the editor).
///
/// [`ActionExecutor`]s run the [system](Action::is_system) kinds; the desktop
/// app carries out the others (navigation, multi actions, OBS).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "camelCase")]
#[non_exhaustive]
pub enum Action {
    /// Press a key combination, e.g. `Ctrl + Alt + T`.
    Hotkey(Hotkey),
    Media(MediaAction),
    Volume(VolumeAction),
    Microphone(MicrophoneAction),
    Open(OpenAction),
    /// Lock the session.
    Lock,
    Navigate(NavigateAction),
    Multi(MultiAction),
    Obs(ObsAction),
}

impl Action {
    /// Short machine-readable name, used in logs and errors.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Hotkey(_) => "hotkey",
            Self::Media(_) => "media",
            Self::Volume(_) => "volume",
            Self::Microphone(_) => "microphone",
            Self::Open(_) => "open",
            Self::Lock => "lock",
            Self::Navigate(_) => "navigate",
            Self::Multi(_) => "multi",
            Self::Obs(_) => "obs",
        }
    }

    /// Run by an [`ActionExecutor`] (blocking, on this PC).
    #[must_use]
    pub const fn is_system(&self) -> bool {
        matches!(
            self,
            Self::Hotkey(_)
                | Self::Media(_)
                | Self::Volume(_)
                | Self::Microphone(_)
                | Self::Open(_)
                | Self::Lock
        )
    }

    /// Checks the parameters before running the action: ranges, lengths and
    /// required values. Page and folder ids are resolved by the desktop app.
    ///
    /// # Errors
    /// [`ActionError::Invalid`] describing the first problem found.
    pub fn validate(&self) -> Result<(), ActionError> {
        self.check(false)
    }

    /// Like [`validate`](Self::validate) for an action being edited: values
    /// not filled in yet (no key, no target, no scene…) are accepted, bounds
    /// are still enforced.
    ///
    /// # Errors
    /// [`ActionError::Invalid`] describing the first problem found.
    pub fn validate_draft(&self) -> Result<(), ActionError> {
        self.check(true)
    }

    fn check(&self, draft: bool) -> Result<(), ActionError> {
        let invalid = |m: String| Err(ActionError::Invalid(m));
        // Missing names pass in drafts; present ones are always bounded.
        let name_ok = |n: &Option<String>| match n.as_deref() {
            None | Some("") => draft,
            Some(n) => !n.trim().is_empty() && n.len() <= MAX_ACTION_NAME,
        };
        match self {
            Self::Hotkey(hotkey) if draft && hotkey.keys.is_empty() => Ok(()),
            Self::Hotkey(hotkey) => hotkey.validate(),
            Self::Media(_) | Self::Microphone(_) | Self::Lock => Ok(()),
            Self::Volume(v) => match (v.command, v.value) {
                (VolumeCommand::Up | VolumeCommand::Down, Some(step))
                    if !(1..=100).contains(&step) =>
                {
                    invalid(format!("volume step {step} % is not within 1–100"))
                }
                (VolumeCommand::Set, None) => invalid("volume level is missing".into()),
                (VolumeCommand::Set, Some(level)) if level > 100 => {
                    invalid(format!("volume level {level} % is above 100"))
                }
                _ => Ok(()),
            },
            Self::Open(o) => {
                if o.target.trim().is_empty() && !(draft && o.target.is_empty()) {
                    invalid("nothing to open".into())
                } else if o.target.len() > MAX_OPEN_TARGET {
                    invalid(format!("open target longer than {MAX_OPEN_TARGET} bytes"))
                } else if o.target.contains('\0') {
                    invalid("open target contains a NUL byte".into())
                } else {
                    Ok(())
                }
            }
            Self::Navigate(n) => match n.to {
                NavigateTo::Page | NavigateTo::Folder if !name_ok(&n.id) => {
                    invalid("navigation needs a page or folder".into())
                }
                _ => Ok(()),
            },
            Self::Multi(m) => {
                if m.steps.is_empty() {
                    return invalid("multi action has no step".into());
                }
                if m.steps.len() > MAX_MULTI_STEPS {
                    return invalid(format!(
                        "multi action has more than {MAX_MULTI_STEPS} steps"
                    ));
                }
                for (i, step) in m.steps.iter().enumerate() {
                    if matches!(step.action, Self::Multi(_)) {
                        return invalid(format!("step {} is a multi action", i + 1));
                    }
                    if step.delay_ms > MAX_MULTI_DELAY_MS {
                        return invalid(format!(
                            "step {} waits more than {MAX_MULTI_DELAY_MS} ms",
                            i + 1
                        ));
                    }
                    step.action
                        .check(draft)
                        .map_err(|e| ActionError::Invalid(format!("step {}: {e}", i + 1)))?;
                }
                Ok(())
            }
            Self::Obs(o) => {
                if o.command.needs_target() && !name_ok(&o.target) {
                    invalid("OBS command needs a scene or source name".into())
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// A key combination. Keys are pressed in order and released in reverse
/// order, so modifiers must come first: `[ControlLeft, AltLeft, KeyT]`.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    /// W3C `KeyboardEvent.code` names. `Key` is generated by a macro, which
    /// typeshare cannot see: the editor gets the list of valid names at runtime.
    #[typeshare(serialized_as = "Vec<String>")]
    pub keys: Vec<Key>,
}

impl Hotkey {
    #[must_use]
    pub fn new(keys: impl Into<Vec<Key>>) -> Self {
        Self { keys: keys.into() }
    }

    /// # Errors
    /// [`ActionError::Invalid`] if the hotkey is empty, too long or repeats a key.
    pub fn validate(&self) -> Result<(), ActionError> {
        if self.keys.is_empty() {
            return Err(ActionError::Invalid("hotkey has no key".into()));
        }
        if self.keys.len() > MAX_HOTKEY_KEYS {
            return Err(ActionError::Invalid(format!(
                "hotkey has {} keys, at most {MAX_HOTKEY_KEYS} allowed",
                self.keys.len()
            )));
        }
        let mut seen = HashSet::new();
        if let Some(dup) = self.keys.iter().find(|k| !seen.insert(**k)) {
            return Err(ActionError::Invalid(format!("hotkey repeats {dup}")));
        }
        Ok(())
    }
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, key) in self.keys.iter().enumerate() {
            if i > 0 {
                f.write_str(" + ")?;
            }
            write!(f, "{key}")?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ActionError {
    /// The action itself is malformed (empty hotkey…).
    #[error("invalid action: {0}")]
    Invalid(String),
    /// This kind of action has no implementation on this platform.
    #[error("action `{action}` is not supported on {platform}")]
    Unsupported {
        action: &'static str,
        platform: &'static str,
    },
    /// A system facility the action needs is not available (e.g. no access to
    /// `/dev/uinput`). The message tells the user how to fix it.
    #[error("{0}")]
    Unavailable(String),
    /// The action was attempted but the system call failed.
    #[error("action failed: {0}")]
    Failed(String),
}

/// Executes actions on this computer.
pub trait ActionExecutor: Send + Sync + std::fmt::Debug {
    /// Executes the action. Blocking.
    ///
    /// # Errors
    /// See [`ActionError`]. The executor has already logged the error.
    fn execute(&self, action: &Action) -> Result<(), ActionError>;
}

/// The executor for the platform this binary was compiled for.
#[must_use]
pub fn platform_executor() -> Box<dyn ActionExecutor> {
    #[cfg(target_os = "linux")]
    let executor = linux::LinuxExecutor::new();
    #[cfg(windows)]
    let executor = win::WindowsExecutor::new();
    #[cfg(not(any(target_os = "linux", windows)))]
    let executor = unsupported::UnsupportedExecutor;
    Box::new(executor)
}

/// Low-level key injection, implemented per platform. Kept separate from
/// [`ActionExecutor`] so the press/release sequencing is shared and testable.
pub(crate) trait KeyInjector {
    fn key_down(&mut self, key: Key) -> Result<(), ActionError>;
    fn key_up(&mut self, key: Key) -> Result<(), ActionError>;
}

/// Presses the keys in order, holds them, then releases them in reverse order.
/// If a press fails, the keys already pressed are released before returning,
/// so a modifier is never left stuck down.
pub(crate) fn run_hotkey(
    injector: &mut impl KeyInjector,
    hotkey: &Hotkey,
    hold: Duration,
) -> Result<(), ActionError> {
    hotkey.validate()?;

    let mut pressed = Vec::with_capacity(hotkey.keys.len());
    let mut result = Ok(());
    for &key in &hotkey.keys {
        match injector.key_down(key) {
            Ok(()) => pressed.push(key),
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    if result.is_ok() && !hold.is_zero() {
        thread::sleep(hold);
    }
    for &key in pressed.iter().rev() {
        let released = injector.key_up(key);
        if result.is_ok() {
            result = released;
        }
    }
    result
}

/// For the kinds a platform executor does not run (yet, or ever: the desktop
/// app carries out navigation, multi actions and OBS).
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn not_yet(action: &Action) -> Result<(), ActionError> {
    if action.is_system() {
        Err(ActionError::Unsupported {
            action: action.kind(),
            platform: std::env::consts::OS,
        })
    } else {
        Err(ActionError::Invalid(format!(
            "{} actions are carried out by the desktop app",
            action.kind()
        )))
    }
}

/// Logs the outcome of an action. Shared by the platform executors.
pub(crate) fn log_outcome(action: &Action, result: &Result<(), ActionError>) {
    match result {
        Ok(()) => tracing::debug!(kind = action.kind(), ?action, "action executed"),
        Err(e @ ActionError::Unsupported { .. }) => tracing::warn!(kind = action.kind(), "{e}"),
        Err(e) => tracing::error!(kind = action.kind(), ?action, "{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records calls, optionally failing on a given key press.
    #[derive(Default)]
    struct FakeInjector {
        log: Vec<String>,
        fail_down_on: Option<Key>,
    }

    impl KeyInjector for FakeInjector {
        fn key_down(&mut self, key: Key) -> Result<(), ActionError> {
            if self.fail_down_on == Some(key) {
                return Err(ActionError::Failed(format!("{key} down")));
            }
            self.log.push(format!("down {key}"));
            Ok(())
        }
        fn key_up(&mut self, key: Key) -> Result<(), ActionError> {
            self.log.push(format!("up {key}"));
            Ok(())
        }
    }

    #[test]
    fn hotkey_presses_in_order_and_releases_in_reverse() {
        let mut fake = FakeInjector::default();
        let hk = Hotkey::new([Key::ControlLeft, Key::AltLeft, Key::KeyT]);
        run_hotkey(&mut fake, &hk, Duration::ZERO).unwrap();
        assert_eq!(
            fake.log,
            [
                "down ControlLeft",
                "down AltLeft",
                "down KeyT",
                "up KeyT",
                "up AltLeft",
                "up ControlLeft"
            ]
        );
    }

    #[test]
    fn failed_press_releases_already_pressed_keys() {
        let mut fake = FakeInjector {
            fail_down_on: Some(Key::KeyT),
            ..Default::default()
        };
        let hk = Hotkey::new([Key::ControlLeft, Key::AltLeft, Key::KeyT]);
        let err = run_hotkey(&mut fake, &hk, Duration::ZERO).unwrap_err();
        assert!(matches!(err, ActionError::Failed(_)));
        assert_eq!(
            fake.log,
            [
                "down ControlLeft",
                "down AltLeft",
                "up AltLeft",
                "up ControlLeft"
            ]
        );
    }

    #[test]
    fn invalid_hotkey_injects_nothing() {
        let mut fake = FakeInjector::default();
        let hk = Hotkey::new([Key::KeyA, Key::KeyA]);
        assert!(run_hotkey(&mut fake, &hk, Duration::ZERO).is_err());
        assert!(fake.log.is_empty());
    }

    #[test]
    fn hotkey_validation() {
        assert!(Hotkey::new([Key::KeyA]).validate().is_ok());
        assert!(Hotkey::new([]).validate().is_err());
        assert!(
            Hotkey::new([Key::ShiftLeft, Key::KeyA, Key::ShiftLeft])
                .validate()
                .is_err()
        );
        let too_long = [
            Key::KeyA,
            Key::KeyB,
            Key::KeyC,
            Key::KeyD,
            Key::KeyE,
            Key::KeyF,
            Key::KeyG,
        ];
        assert!(Hotkey::new(too_long).validate().is_err());
    }

    #[test]
    fn hotkey_display() {
        let hk = Hotkey::new([Key::ControlLeft, Key::KeyC]);
        assert_eq!(hk.to_string(), "ControlLeft + KeyC");
    }
}
