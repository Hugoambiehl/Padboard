//! Authenticode (Windows): is a file validly signed, and by whom?

use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, HWND, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Cryptography::{CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW};
use windows::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData,
    WinVerifyTrust,
};
use windows::core::PCWSTR;

/// The name of the certificate that validly signed `path` (Authenticode,
/// checked by Windows up to a trusted root), e.g. "Google LLC".
///
/// # Errors
/// Not signed, a signature Windows does not trust, or unreadable.
pub fn signer(path: &Path) -> Result<String, String> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: struct_size::<WINTRUST_FILE_INFO>(),
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: struct_size::<WINTRUST_DATA>(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: `data` and `file` (and the path they point to) outlive both
    // calls; the state opened by VERIFY is closed below whatever happens.
    let status = unsafe {
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0),
            &raw mut action,
            (&raw mut data).cast(),
        )
    };
    let name = if status == 0 {
        // SAFETY: the state data is valid until WTD_STATEACTION_CLOSE.
        unsafe { signer_name(data.hWVTStateData) }
    } else {
        Err(format!(
            "not validly signed (WinVerifyTrust 0x{status:08X})"
        ))
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: closes the state opened above.
    unsafe {
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0),
            &raw mut action,
            (&raw mut data).cast(),
        );
    }
    name
}

/// `cbStruct` of a Win32 structure (always far below `u32::MAX`).
fn struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}

/// The simple name of the leaf certificate of the first signer.
unsafe fn signer_name(state: HANDLE) -> Result<String, String> {
    // SAFETY: the caller passes a live WinVerifyTrust state; each pointer is
    // checked before use.
    unsafe {
        let provider = WTHelperProvDataFromStateData(state);
        if provider.is_null() {
            return Err("no provider data".into());
        }
        let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
        if signer.is_null() {
            return Err("no signer".into());
        }
        let cert = WTHelperGetProvCertFromChain(signer, 0);
        if cert.is_null() || (*cert).pCert.is_null() {
            return Err("no signing certificate".into());
        }
        let mut buffer = [0u16; 256];
        let len = CertGetNameStringW(
            (*cert).pCert,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut buffer),
        );
        let len = usize::try_from(len).unwrap_or(0).saturating_sub(1); // without the final NUL
        Ok(String::from_utf16_lossy(&buffer[..len.min(buffer.len())]))
    }
}
