//! "Supprimer adb": removes the downloaded copy without ever leaving half of
//! it behind (#35).
//!
//! Windows keeps a running program's file locked, and the copy's `adb.exe`
//! may still run for a moment after its server was stopped (a command in
//! flight: `dpm list-owners`, `adb reverse`…). So the folder is first moved
//! aside in one step (refused as a whole while a file inside is in use,
//! then retried a few times), and only then deleted. Either the copy is
//! gone from its place, or it is still whole and usable.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::PlatformToolsError;

/// Moves aside: one attempt, then this many more while a file is in use.
const ATTEMPTS: u32 = 20;
/// Between two attempts (so about 5 s in all).
const PAUSE: Duration = Duration::from_millis(250);
/// Appended to the folder's name while it is being deleted.
const LEFTOVER_SUFFIX: &str = ".removing";

/// The file system operations the removal needs (faked in tests).
pub(super) trait RemovalFs {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, dir: &Path) -> io::Result<()>;
}

struct RealFs;

impl RemovalFs for RealFs {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(dir)
    }
}

/// Removes the downloaded copy in `dir` (its adb server stopped first, see
/// `lib.rs`). Blocking: up to about 5 s while a file is in use.
///
/// # Errors
/// [`PlatformToolsError::InUse`] when a file stayed in use (the copy is
/// left whole), [`PlatformToolsError::Install`] for any other failure.
pub fn remove(dir: &Path) -> Result<(), PlatformToolsError> {
    remove_with(dir, &RealFs, ATTEMPTS, &std::thread::sleep)
}

/// Deletes what an earlier removal could not finish deleting (logged only).
pub fn discard_leftover(dir: &Path) {
    discard_leftover_with(dir, &RealFs);
}

fn leftover(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(LEFTOVER_SUFFIX);
    dir.with_file_name(name)
}

fn discard_leftover_with(dir: &Path, fs: &impl RemovalFs) {
    let leftover = leftover(dir);
    match fs.remove_dir_all(&leftover) {
        Ok(()) => {
            tracing::info!(path = %leftover.display(), "leftover of an earlier removal deleted")
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(path = %leftover.display(), "leftover of an earlier removal kept: {e}")
        }
    }
}

/// A file is in use by a running program (Windows: access denied, sharing
/// or lock violation), worth trying again in a moment.
fn in_use(e: &io::Error) -> bool {
    const SHARING_VIOLATION: i32 = 32;
    const LOCK_VIOLATION: i32 = 33;
    matches!(
        e.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ResourceBusy
    ) || (cfg!(windows) && matches!(e.raw_os_error(), Some(SHARING_VIOLATION | LOCK_VIOLATION)))
}

pub(super) fn remove_with(
    dir: &Path,
    fs: &impl RemovalFs,
    attempts: u32,
    sleep: &dyn Fn(Duration),
) -> Result<(), PlatformToolsError> {
    discard_leftover_with(dir, fs);
    let aside = leftover(dir);
    let mut tries = 0;
    loop {
        tries += 1;
        match fs.rename(dir, &aside) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) if in_use(&e) && tries <= attempts => {
                tracing::info!(tries, "platform-tools in use, trying again: {e}");
                sleep(PAUSE);
            }
            Err(e) if in_use(&e) => {
                tracing::warn!(tries, "platform-tools still in use, left whole: {e}");
                return Err(PlatformToolsError::InUse {
                    detail: e.to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("platform-tools cannot be removed, left whole: {e}");
                return Err(PlatformToolsError::Install {
                    detail: e.to_string(),
                });
            }
        }
    }
    // Moved aside: the copy is no longer where adb is looked for. Deleting
    // can only fail on a file that is in use again; the next removal or
    // installation deletes the rest.
    match fs.remove_dir_all(&aside) {
        Ok(()) => tracing::info!(path = %dir.display(), "platform-tools removed"),
        Err(e) => tracing::warn!(
            path = %aside.display(),
            "platform-tools moved aside, not fully deleted yet: {e}"
        ),
    }
    Ok(())
}

/// Calls `done` until it says yes, at most `attempts` times, `pause` apart.
/// Returns whether it did.
pub fn wait_until(
    mut done: impl FnMut() -> bool,
    attempts: u32,
    pause: Duration,
    sleep: &dyn Fn(Duration),
) -> bool {
    for attempt in 1..=attempts {
        if done() {
            return true;
        }
        if attempt < attempts {
            sleep(pause);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// Renames fail with `rename_error` for the first `busy_renames` calls.
    #[derive(Default)]
    struct FakeFs {
        busy_renames: Cell<u32>,
        rename_error: Option<io::ErrorKind>,
        remove_fails: bool,
        calls: RefCell<Vec<String>>,
        /// The copy, at its place.
        present: Cell<bool>,
        /// The copy, moved aside.
        aside: Cell<bool>,
    }

    impl FakeFs {
        fn installed() -> Self {
            Self {
                present: Cell::new(true),
                rename_error: Some(io::ErrorKind::PermissionDenied),
                ..Self::default()
            }
        }
    }

    impl RemovalFs for FakeFs {
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("rename {} {}", slashes(from), slashes(to)));
            if !self.present.get() {
                return Err(io::ErrorKind::NotFound.into());
            }
            if self.busy_renames.get() > 0 {
                self.busy_renames.set(self.busy_renames.get() - 1);
                return Err(self.rename_error.unwrap().into());
            }
            self.present.set(false);
            self.aside.set(true);
            Ok(())
        }

        fn remove_dir_all(&self, dir: &Path) -> io::Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("remove {}", slashes(dir)));
            if !self.aside.get() {
                return Err(io::ErrorKind::NotFound.into());
            }
            if self.remove_fails {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            self.aside.set(false);
            Ok(())
        }
    }

    /// The path with `/` only, so the expected calls read the same on Windows.
    fn slashes(path: &Path) -> String {
        path.display().to_string().replace('\\', "/")
    }

    fn renames(fs: &FakeFs) -> usize {
        fs.calls
            .borrow()
            .iter()
            .filter(|c| c.starts_with("rename"))
            .count()
    }

    #[test]
    fn a_free_copy_is_moved_aside_then_deleted() {
        let fs = FakeFs::installed();
        let slept = Cell::new(0);
        remove_with(Path::new("/x/pt"), &fs, 3, &|_| slept.set(slept.get() + 1)).unwrap();
        assert_eq!(
            *fs.calls.borrow(),
            [
                "remove /x/pt.removing",
                "rename /x/pt /x/pt.removing",
                "remove /x/pt.removing"
            ]
        );
        assert_eq!(slept.get(), 0);
    }

    #[test]
    fn a_file_in_use_for_a_moment_is_waited_for() {
        let fs = FakeFs::installed();
        fs.busy_renames.set(2);
        let slept = Cell::new(0);
        remove_with(Path::new("/x/pt"), &fs, 3, &|d| {
            assert_eq!(d, PAUSE);
            slept.set(slept.get() + 1);
        })
        .unwrap();
        assert_eq!(renames(&fs), 3);
        assert_eq!(slept.get(), 2);
        assert!(!fs.present.get() && !fs.aside.get());
    }

    #[test]
    fn a_file_still_in_use_leaves_the_copy_whole_with_a_clear_error() {
        let fs = FakeFs::installed();
        fs.busy_renames.set(100);
        let error = remove_with(Path::new("/x/pt"), &fs, 3, &|_| {}).unwrap_err();
        assert!(
            matches!(error, PlatformToolsError::InUse { .. }),
            "{error:?}"
        );
        // One attempt, then three more; never deleted.
        assert_eq!(renames(&fs), 4);
        assert!(fs.present.get());
        assert!(
            !fs.calls
                .borrow()
                .iter()
                .skip(1)
                .any(|c| c.starts_with("remove"))
        );
    }

    #[test]
    fn another_failure_is_not_retried() {
        let fs = FakeFs {
            rename_error: Some(io::ErrorKind::InvalidInput),
            ..FakeFs::installed()
        };
        fs.busy_renames.set(1);
        let error = remove_with(Path::new("/x/pt"), &fs, 3, &|_| panic!("no pause")).unwrap_err();
        assert!(
            matches!(error, PlatformToolsError::Install { .. }),
            "{error:?}"
        );
        assert_eq!(renames(&fs), 1);
        assert!(fs.present.get());
    }

    #[test]
    fn nothing_installed_is_fine() {
        let fs = FakeFs::default();
        remove_with(Path::new("/x/pt"), &fs, 3, &|_| panic!("no pause")).unwrap();
    }

    #[test]
    fn a_copy_moved_aside_but_not_deleted_counts_as_removed() {
        let fs = FakeFs {
            remove_fails: true,
            ..FakeFs::installed()
        };
        remove_with(Path::new("/x/pt"), &fs, 3, &|_| {}).unwrap();
        assert!(!fs.present.get());
        assert!(fs.aside.get());
    }

    #[test]
    fn real_folders_are_removed_and_leftovers_discarded() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("platform-tools");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("adb.exe"), b"x").unwrap();
        std::fs::write(dir.join("sub/f"), b"x").unwrap();
        let stale = tmp.path().join("platform-tools.removing");
        std::fs::create_dir(&stale).unwrap();
        std::fs::write(stale.join("old"), b"x").unwrap();

        remove(&dir).unwrap();
        assert!(!dir.exists());
        assert!(!stale.exists());
        // Again: nothing there, nothing to do.
        remove(&dir).unwrap();

        std::fs::create_dir(&stale).unwrap();
        discard_leftover(&dir);
        assert!(!stale.exists());
    }

    #[test]
    fn in_use_errors() {
        assert!(in_use(&io::ErrorKind::PermissionDenied.into()));
        assert!(in_use(&io::ErrorKind::ResourceBusy.into()));
        assert!(!in_use(&io::ErrorKind::NotFound.into()));
        assert!(!in_use(&io::ErrorKind::InvalidInput.into()));
        #[cfg(windows)]
        {
            assert!(in_use(&io::Error::from_raw_os_error(5)));
            assert!(in_use(&io::Error::from_raw_os_error(32)));
            assert!(in_use(&io::Error::from_raw_os_error(33)));
        }
    }

    #[test]
    fn waiting_stops_as_soon_as_it_is_done_or_after_the_attempts() {
        let calls = Cell::new(0);
        let pauses = Cell::new(0);
        let sleep = |_| pauses.set(pauses.get() + 1);
        assert!(wait_until(
            || {
                calls.set(calls.get() + 1);
                calls.get() == 3
            },
            10,
            Duration::from_millis(1),
            &sleep
        ));
        assert_eq!((calls.get(), pauses.get()), (3, 2));

        calls.set(0);
        pauses.set(0);
        assert!(!wait_until(
            || {
                calls.set(calls.get() + 1);
                false
            },
            4,
            Duration::from_millis(1),
            &sleep
        ));
        assert_eq!((calls.get(), pauses.get()), (4, 3));
    }
}
