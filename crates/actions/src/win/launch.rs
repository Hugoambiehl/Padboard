//! `open` and `lock` on Windows.

use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use crate::ActionError;

/// `%USERPROFILE%\Documents` → `C:\Users\…\Documents`.
fn expand(target: &str) -> String {
    let source = HSTRING::from(target);
    let mut buffer = vec![0u16; 4096];
    // SAFETY: `source` is NUL-terminated, `buffer` is writable for its length.
    let len = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut buffer)) };
    match usize::try_from(len) {
        Ok(n) if n > 0 && n <= buffer.len() => String::from_utf16_lossy(&buffer[..n - 1]),
        _ => target.to_owned(),
    }
}

/// Opens a URL, a document, a folder or a program with its default handler.
pub fn open(target: &str) -> Result<(), ActionError> {
    let file = HSTRING::from(expand(target.trim()));
    // SAFETY: every string is a NUL-terminated HSTRING alive for the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success (documented HINSTANCE quirk).
    if result.0 as usize > 32 {
        Ok(())
    } else {
        Err(ActionError::Failed(format!(
            "cannot open {target}: {}",
            windows::core::Error::from_thread()
        )))
    }
}

pub fn lock() -> Result<(), ActionError> {
    // SAFETY: no arguments; fails only outside an interactive session.
    unsafe { LockWorkStation() }.map_err(|e| ActionError::Failed(format!("LockWorkStation: {e}")))
}
