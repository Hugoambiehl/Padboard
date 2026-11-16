//! Starting Padboard with the session: an entry that runs the app with
//! `--hidden` (Windows: the `Run` registry key; Linux: a `.desktop` file in
//! `~/.config/autostart`).
//!
//! `auto-launch` writes the entry, but the program path is quoted here:
//! `auto-launch` writes it as is, and an unquoted path with spaces
//! (`C:\Program Files\…`) in a `Run` value is the classic unquoted path flaw
//! (Windows tries `C:\Program.exe` first).
//!
//! The entry is always the user's own (`HKCU` on Windows, never `HKLM`, even
//! when the app runs elevated) and a `.desktop` file on Linux, not a systemd
//! unit: the modes are set explicitly, not left to the crate's defaults.
//!
//! The user's choice is also kept in `preferences.json` (`autostart`), so an
//! entry removed behind Padboard's back is noticed at launch: Bitdefender's
//! ATC remediation deleted the `Run` value (and so does the uninstaller of a
//! version installed over without `/UPDATE`). On Windows it is written again,
//! the `Run` value only: `StartupApproved` (the Task Manager's switch) is
//! never touched there, so Padboard disabled in the Task Manager stays
//! disabled. On Linux a missing `.desktop` file is the user's doing (GNOME's
//! startup applications remove it): the choice follows it.

use std::sync::atomic::{AtomicBool, Ordering};

use auto_launch::{AutoLaunch, AutoLaunchBuilder, LinuxLaunchMode, WindowsEnableMode};

/// The entry's name (`Run` value, `<name>.desktop`).
const NAME: &str = "Padboard";

/// The system's entry, or a flag in test mode (tests never touch the real
/// autostart).
pub enum Autostart {
    System(Result<AutoLaunch, String>),
    Simulated(AtomicBool),
}

/// What to do at launch, from the user's recorded choice and whether the
/// entry is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtLaunch {
    /// No choice recorded (saved by an older version): the entry tells.
    Record(bool),
    /// Chosen, but the entry is gone: write it again.
    Restore,
    /// Chosen and there: rewrite it (the program may have moved).
    Refresh,
    /// The entry was removed by the user (Linux): the choice follows.
    Forget,
    Nothing,
}

/// The decision, kept apart to be tested. `restores` is whether a missing
/// entry is written again (Windows; test mode, to test it).
#[must_use]
pub fn at_launch(chosen: Option<bool>, registered: bool, restores: bool) -> AtLaunch {
    match (chosen, registered) {
        (None, registered) => AtLaunch::Record(registered),
        (Some(true), true) => AtLaunch::Refresh,
        (Some(true), false) if restores => AtLaunch::Restore,
        (Some(true), false) => AtLaunch::Forget,
        (Some(false), _) => AtLaunch::Nothing,
    }
}

impl Autostart {
    pub fn system() -> Self {
        let entry = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| {
                let exe = exe.to_str().ok_or("program path is not UTF-8")?.to_owned();
                AutoLaunchBuilder::new()
                    .set_app_name(NAME)
                    .set_app_path(&quote(&exe))
                    .set_args(&[crate::tray::HIDDEN_ARG])
                    .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
                    .set_linux_launch_mode(LinuxLaunchMode::XdgAutostart)
                    .build()
                    .map_err(|e| e.to_string())
            });
        if let Err(e) = &entry {
            tracing::error!("start with the session unavailable: {e}");
        }
        Self::System(entry)
    }

    pub fn simulated() -> Self {
        Self::Simulated(AtomicBool::new(false))
    }

    pub fn is_enabled(&self) -> Result<bool, String> {
        match self {
            Self::System(entry) => entry
                .as_ref()
                .map_err(Clone::clone)?
                .is_enabled()
                .map_err(|e| e.to_string()),
            Self::Simulated(on) => Ok(on.load(Ordering::Relaxed)),
        }
    }

    pub fn set(&self, enabled: bool) -> Result<(), String> {
        match self {
            Self::System(entry) => {
                let entry = entry.as_ref().map_err(Clone::clone)?;
                if enabled {
                    entry.enable()
                } else {
                    entry.disable()
                }
                .map_err(|e| e.to_string())
            }
            Self::Simulated(on) => {
                on.store(enabled, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    /// Whether the entry exists, whatever the Task Manager's switch says
    /// (`is_enabled` is false when Padboard is disabled there).
    fn is_registered(&self) -> Result<bool, String> {
        match self {
            #[cfg(windows)]
            Self::System(_) => windows::is_registered(),
            #[cfg(not(windows))]
            Self::System(_) => self.is_enabled(),
            Self::Simulated(on) => Ok(on.load(Ordering::Relaxed)),
        }
    }

    /// Writes the entry again, without touching `StartupApproved`.
    fn rewrite(&self) -> Result<(), String> {
        match self {
            #[cfg(windows)]
            Self::System(entry) => {
                let entry = entry.as_ref().map_err(Clone::clone)?;
                windows::write(&format!(
                    "{} {}",
                    entry.get_app_path(),
                    entry.get_args().join(" ")
                ))
            }
            #[cfg(not(windows))]
            Self::System(_) => self.set(true),
            Self::Simulated(on) => {
                on.store(true, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    /// Checks the entry against the user's choice (`chosen`, from
    /// `preferences.json`) and returns the choice to record if it changed.
    /// A refresh also fixes entries written by older versions (unquoted
    /// path) and follows the program if it moved.
    pub fn at_launch(&self, chosen: Option<bool>) -> Option<bool> {
        let registered = match self.is_registered() {
            Ok(registered) => registered,
            Err(e) => {
                tracing::warn!("cannot read the start with the session entry: {e}");
                return None;
            }
        };
        let restores = cfg!(windows) || matches!(self, Self::Simulated(_));
        match at_launch(chosen, registered, restores) {
            AtLaunch::Record(on) => {
                tracing::info!(on, "start with the session: choice recorded from the entry");
                Some(on)
            }
            AtLaunch::Restore => {
                match self.rewrite() {
                    Ok(()) => tracing::warn!(
                        "start with the session: the entry had disappeared \
                         (removed by another program), written again"
                    ),
                    Err(e) => tracing::error!(
                        "start with the session: the entry had disappeared and cannot be written again: {e}"
                    ),
                }
                None
            }
            AtLaunch::Refresh => {
                match self.rewrite() {
                    Ok(()) => tracing::info!("start with the session: entry refreshed"),
                    Err(e) => {
                        tracing::warn!("cannot refresh the start with the session entry: {e}");
                    }
                }
                None
            }
            AtLaunch::Forget => {
                tracing::info!(
                    "start with the session: entry removed outside Padboard, turned off"
                );
                Some(false)
            }
            AtLaunch::Nothing => None,
        }
    }
}

/// The `Run` value, read and written directly (`auto-launch`'s `enable`
/// also marks it enabled in `StartupApproved`, which would undo the Task
/// Manager's switch).
#[cfg(windows)]
mod windows {
    use windows_registry::CURRENT_USER;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// `HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`.
    const NOT_FOUND: i32 = 0x8007_0002_u32.cast_signed();

    pub fn is_registered() -> Result<bool, String> {
        match CURRENT_USER
            .open(RUN)
            .and_then(|key| key.get_string(super::NAME))
        {
            Ok(_) => Ok(true),
            Err(e) if e.code().0 == NOT_FOUND => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn write(command: &str) -> Result<(), String> {
        CURRENT_USER
            .create(RUN)
            .and_then(|key| key.set_string(super::NAME, command))
            .map_err(|e| e.to_string())
    }
}

/// The program path as the entry's command needs it.
#[cfg(windows)]
fn quote(path: &str) -> String {
    // Windows paths cannot contain `"`.
    format!("\"{path}\"")
}

/// The program path quoted for a desktop entry's `Exec` key: inside double
/// quotes, `"`, `` ` ``, `$` and `\` are escaped with a backslash, then the
/// value itself escapes backslashes again; `%` is a field code, doubled.
#[cfg(not(windows))]
fn quote(path: &str) -> String {
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn desktop_entry_paths_are_quoted() {
        assert_eq!(
            quote("/usr/bin/padboard-desktop"),
            "\"/usr/bin/padboard-desktop\""
        );
        assert_eq!(
            quote("/opt/My Apps/padboard-desktop"),
            "\"/opt/My Apps/padboard-desktop\""
        );
        assert_eq!(quote("/a$b/c\"d"), "\"/a\\\\$b/c\\\\\"d\"");
        assert_eq!(quote("/a\\b"), "\"/a\\\\\\\\b\"");
        assert_eq!(quote("/100%/x"), "\"/100%%/x\"");
    }

    #[cfg(windows)]
    #[test]
    fn run_value_paths_are_quoted() {
        assert_eq!(
            quote(r"D:\Programme Files\Padboard\padboard-desktop.exe"),
            r#""D:\Programme Files\Padboard\padboard-desktop.exe""#
        );
    }

    #[test]
    fn simulated_entry_is_a_flag() {
        let entry = Autostart::simulated();
        assert_eq!(entry.is_enabled(), Ok(false));
        entry.set(true).unwrap();
        assert_eq!(entry.is_enabled(), Ok(true));
    }

    #[test]
    fn the_choice_and_the_entry_decide_at_launch() {
        // Saved by an older version: the entry tells.
        assert_eq!(at_launch(None, true, true), AtLaunch::Record(true));
        assert_eq!(at_launch(None, false, true), AtLaunch::Record(false));
        assert_eq!(at_launch(Some(true), true, true), AtLaunch::Refresh);
        // Removed by another program (Windows): written again.
        assert_eq!(at_launch(Some(true), false, true), AtLaunch::Restore);
        // Removed by the user (Linux): the choice follows.
        assert_eq!(at_launch(Some(true), false, false), AtLaunch::Forget);
        // Off: whatever is there is left alone.
        assert_eq!(at_launch(Some(false), true, true), AtLaunch::Nothing);
        assert_eq!(at_launch(Some(false), false, false), AtLaunch::Nothing);
    }

    #[test]
    fn a_missing_entry_is_written_again() {
        let entry = Autostart::simulated();
        // Nothing recorded yet: the entry's absence is recorded.
        assert_eq!(entry.at_launch(None), Some(false));
        entry.set(true).unwrap();
        assert_eq!(entry.at_launch(Some(true)), None);
        // Removed behind Padboard's back.
        entry.set(false).unwrap();
        assert_eq!(entry.at_launch(Some(true)), None);
        assert_eq!(entry.is_enabled(), Ok(true));
        // Turned off by the user: not written again.
        entry.set(false).unwrap();
        assert_eq!(entry.at_launch(Some(false)), None);
        assert_eq!(entry.is_enabled(), Ok(false));
    }
}
