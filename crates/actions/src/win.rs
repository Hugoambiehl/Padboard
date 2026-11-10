//! Windows implementation.
//!
//! Keys are injected with `SendInput`, as scancodes when possible so the result
//! does not depend on the keyboard layout (and games that read scancodes see
//! them), as virtual-key codes for media keys, Pause and NumLock.
//!
//! Limitation (UIPI): a non-elevated process cannot inject input into an
//! elevated window (e.g. an app run as administrator).

use std::mem::size_of;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, SendInput, VIRTUAL_KEY,
};

use crate::{
    Action, ActionError, ActionExecutor, HOTKEY_HOLD, Key, KeyInjector, WinKey, log_outcome,
    run_hotkey,
};

/// Executes actions on Windows.
#[derive(Debug, Default)]
pub struct WindowsExecutor;

impl WindowsExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ActionExecutor for WindowsExecutor {
    fn execute(&self, action: &Action) -> Result<(), ActionError> {
        let result = match action {
            Action::Hotkey(hotkey) => run_hotkey(&mut SendInputInjector, hotkey, HOTKEY_HOLD),
        };
        log_outcome(action, &result);
        result
    }
}

struct SendInputInjector;

impl SendInputInjector {
    fn send(key: Key, up: bool) -> Result<(), ActionError> {
        let mut flags = if up {
            KEYEVENTF_KEYUP
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        let (vk, scan) = match key.win() {
            WinKey::Scan { code, extended } => {
                flags |= KEYEVENTF_SCANCODE;
                if extended {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                (VIRTUAL_KEY(0), code)
            }
            WinKey::Vk(code) => (VIRTUAL_KEY(code), 0),
        };
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let size = size_of::<INPUT>() as i32;
        // SAFETY: `input` is a fully initialized keyboard INPUT and `size`
        // is the size of the INPUT structure, as SendInput requires.
        let sent = unsafe { SendInput(&[input], size) };
        if sent == 1 {
            Ok(())
        } else {
            Err(ActionError::Failed(format!(
                "SendInput rejected {key} ({}): {}",
                if up { "up" } else { "down" },
                windows::core::Error::from_thread()
            )))
        }
    }
}

impl KeyInjector for SendInputInjector {
    fn key_down(&mut self, key: Key) -> Result<(), ActionError> {
        Self::send(key, false)
    }

    fn key_up(&mut self, key: Key) -> Result<(), ActionError> {
        Self::send(key, true)
    }
}
