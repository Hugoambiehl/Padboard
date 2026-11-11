//! `open` and `lock` on Linux. Programs are started without a shell (the
//! target is a single argument), in their own process group, so they outlive
//! Padboard and never see its terminal signals.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ActionError;

/// How a target is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// `https://…`, `mailto:…`: the default handler (`xdg-open`).
    Url(String),
    /// An application entry (`.desktop`): `gio launch`.
    DesktopEntry(PathBuf),
    /// A program: started directly.
    Program(PathBuf),
    /// A file or a folder: the default application (`xdg-open`).
    Document(PathBuf),
}

/// Classifies `target`. `~/` is the home folder; a bare name (`firefox`) is
/// looked up in `PATH`.
///
/// # Errors
/// [`ActionError::Failed`] if nothing matches.
pub fn classify(
    target: &str,
    home: Option<&Path>,
    path: Option<&str>,
) -> Result<Launch, ActionError> {
    let target = target.trim();
    if target.contains("://") || target.starts_with("mailto:") {
        return Ok(Launch::Url(target.to_owned()));
    }
    let file = match (target.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(target),
    };
    if !target.contains('/') {
        // A command name, found in PATH.
        if let Some(program) = path
            .into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join(target))
            .find(|p| is_executable(p))
        {
            return Ok(Launch::Program(program));
        }
    }
    if !file.exists() {
        return Err(ActionError::Failed(format!(
            "{} does not exist",
            file.display()
        )));
    }
    if file.extension().is_some_and(|e| e == "desktop") {
        Ok(Launch::DesktopEntry(file))
    } else if is_executable(&file) {
        Ok(Launch::Program(file))
    } else {
        Ok(Launch::Document(file))
    }
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Opens `target` (see [`classify`]).
///
/// # Errors
/// If the target does not exist or the launcher cannot be started.
pub fn open(target: &str) -> Result<(), ActionError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path = std::env::var("PATH").ok();
    let mut command = match classify(target, home.as_deref(), path.as_deref())? {
        Launch::Url(url) => with_arg("xdg-open", url),
        Launch::Document(file) => with_arg("xdg-open", file),
        Launch::DesktopEntry(file) => {
            let mut c = Command::new("gio");
            c.arg("launch").arg(file);
            c
        }
        Launch::Program(program) => Command::new(program),
    };
    spawn_detached(&mut command)
}

fn with_arg(program: &str, arg: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(program);
    c.arg(arg);
    c
}

/// Starts the command in its own process group and reaps it in the
/// background (no zombie, no wait on the caller's thread).
fn spawn_detached(command: &mut Command) -> Result<(), ActionError> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                ActionError::Unavailable(format!("{program} is not installed"))
            }
            _ => ActionError::Failed(format!("cannot start {program}: {e}")),
        })?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Locks the session (`loginctl lock-session`: GNOME, KDE and most desktops).
///
/// # Errors
/// If `loginctl` is missing or refuses.
pub fn lock() -> Result<(), ActionError> {
    let output = Command::new("loginctl")
        .arg("lock-session")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| ActionError::Unavailable(format!("loginctl: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ActionError::Failed(format!(
            "loginctl lock-session: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_classified() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let doc = home.join("notes.txt");
        std::fs::write(&doc, "x").unwrap();
        let entry = home.join("app.desktop");
        std::fs::write(&entry, "[Desktop Entry]").unwrap();
        let script = home.join("run.sh");
        std::fs::write(&script, "#!/bin/sh").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let tool = bin.join("mytool");
        std::fs::write(&tool, "#!/bin/sh").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = bin.to_str().unwrap();

        let c = |t: &str| classify(t, Some(home), Some(path));
        assert_eq!(
            c("https://example.org").unwrap(),
            Launch::Url("https://example.org".into())
        );
        assert_eq!(
            c("mailto:a@b.c").unwrap(),
            Launch::Url("mailto:a@b.c".into())
        );
        assert_eq!(c("~/notes.txt").unwrap(), Launch::Document(doc));
        assert_eq!(
            c(home.to_str().unwrap()).unwrap(),
            Launch::Document(home.to_owned())
        );
        assert_eq!(
            c(entry.to_str().unwrap()).unwrap(),
            Launch::DesktopEntry(entry.clone())
        );
        assert_eq!(
            c(script.to_str().unwrap()).unwrap(),
            Launch::Program(script)
        );
        assert_eq!(c("mytool").unwrap(), Launch::Program(tool));
        assert!(matches!(c("~/missing"), Err(ActionError::Failed(_))));
        assert!(matches!(c("nosuchcommand"), Err(ActionError::Failed(_))));
    }
}
