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

    /// Rewrites an existing entry, so it follows the program if it moved and
    /// entries written by older versions (unquoted path) are fixed.
    pub fn refresh(&self) {
        if matches!(self, Self::System(_)) && self.is_enabled() == Ok(true) {
            match self.set(true) {
                Ok(()) => tracing::info!("start with the session: entry refreshed"),
                Err(e) => tracing::warn!("cannot refresh the start with the session entry: {e}"),
            }
        }
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
        entry.refresh();
    }
}
