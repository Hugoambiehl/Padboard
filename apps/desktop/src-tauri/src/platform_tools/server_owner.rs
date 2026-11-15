//! Which program runs the adb server, before "Supprimer adb" stops it.
//!
//! Every adb on the PC talks to the one server on port 5037, whoever started
//! it: `adb kill-server` from our copy would stop another program's server
//! (seen with Touch Portal's adb 28.0.3, first in `PATH`, same protocol 41
//! as ours, so not a version conflict). Only a server started from our own
//! copy is stopped.

use std::path::{Path, PathBuf};

/// `a` and `b` are the same executable (Windows paths: case does not
/// matter, `/` and `\` are the same).
#[must_use]
pub fn same_program(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b))
        && a == b
    {
        return true;
    }
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

/// The executable of the process listening on `127.0.0.1:port` (the adb
/// server), if any and if Windows tells it.
#[cfg(windows)]
#[must_use]
pub fn server_owner(port: u16) -> Option<PathBuf> {
    let pid = listener_pid(port)?;
    match process_path(pid) {
        Ok(path) => Some(path),
        Err(e) => {
            tracing::warn!(pid, "adb server: its program is unknown: {e}");
            None
        }
    }
}

#[cfg(not(windows))]
#[must_use]
pub fn server_owner(_port: u16) -> Option<PathBuf> {
    None
}

/// The process listening on TCP `port` (IPv4), from the system's table.
#[cfg(windows)]
fn listener_pid(port: u16) -> Option<u32> {
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    const AF_INET: u32 = 2;

    let mut size = 0u32;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..4 {
        let ptr = if buf.is_empty() {
            None
        } else {
            Some(buf.as_mut_ptr().cast())
        };
        // SAFETY: `buf` holds `size` bytes (rounded up to u64 words, so
        // aligned for the table), or nothing on the first call.
        let rc = unsafe {
            GetExtendedTcpTable(
                ptr,
                &raw mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if rc == NO_ERROR.0 && !buf.is_empty() {
            break;
        }
        if rc != ERROR_INSUFFICIENT_BUFFER.0 && rc != NO_ERROR.0 {
            tracing::warn!(rc, "adb server: the TCP table is unreadable");
            return None;
        }
        buf = vec![0u64; (size as usize).div_ceil(8) + 1];
    }
    if buf.is_empty() {
        return None;
    }
    let bytes = buf.len() * 8;
    let base = buf.as_ptr().cast::<u8>();
    // SAFETY: the table starts with its entry count (u32), then its rows
    // (aligned to 4 bytes); never read past the buffer.
    let count = unsafe { base.cast::<u32>().read_unaligned() } as usize;
    let row = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
    (0..count)
        .map(|i| 4 + i * row)
        .take_while(|offset| offset + row <= bytes)
        .map(|offset| unsafe {
            base.add(offset)
                .cast::<MIB_TCPROW_OWNER_PID>()
                .read_unaligned()
        })
        .find(|r| u16::from_be((r.dwLocalPort & 0xffff) as u16) == port)
        .map(|r| r.dwOwningPid)
}

#[cfg(windows)]
fn process_path(pid: u32) -> windows::core::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;

    const MAX: u32 = 32_768;

    // SAFETY: plain calls; the handle is closed below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? };
    let mut name = vec![0u16; MAX as usize];
    let mut len = MAX;
    // SAFETY: `name` holds `len` UTF-16 units.
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &raw mut len,
        )
    };
    // SAFETY: opened above, closed once.
    let _ = unsafe { CloseHandle(handle) };
    result?;
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &name[..len as usize],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_program_whatever_the_case_and_slashes() {
        let ours = Path::new(
            r"C:\Users\Hugo\AppData\Local\io.github.hugoambiehl.padboard\platform-tools\adb.exe",
        );
        assert!(same_program(
            Path::new(
                "c:/users/hugo/appdata/local/io.github.hugoambiehl.padboard/platform-tools/ADB.EXE"
            ),
            ours
        ));
        assert!(!same_program(
            Path::new(r"C:\Program Files (x86)\Touch Portal\plugins\adb\platform-tools\adb.exe"),
            ours
        ));
        assert!(!same_program(
            Path::new(r"C:\Android\platform-tools\adb.exe"),
            ours
        ));
    }
}
