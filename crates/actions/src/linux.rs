//! Linux implementation.
//!
//! Keyboard injection goes through a uinput virtual keyboard: it works the same
//! on X11 and Wayland, for every application, because events enter the kernel
//! input stack like a real keyboard. Access to `/dev/uinput` is granted to the
//! active session user by a udev rule (`TAG+="uaccess"`, see the README).

use std::fmt;
use std::io;
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode};

use crate::{
    Action, ActionError, ActionExecutor, HOTKEY_HOLD, Key, KeyInjector, log_outcome, run_hotkey,
};

const DEVICE_NAME: &str = "Padboard virtual keyboard";

/// Time for the compositor / X server to notice a new input device. Events
/// emitted before that are silently dropped.
const DEVICE_SETTLE: Duration = Duration::from_millis(300);

/// Executes actions on Linux.
///
/// The virtual keyboard is created eagerly; if that fails (missing udev rule),
/// creation is retried on each action, so fixing the permission does not
/// require restarting the app.
pub struct LinuxExecutor {
    keyboard: Mutex<Option<VirtualDevice>>,
}

impl LinuxExecutor {
    #[must_use]
    pub fn new() -> Self {
        let keyboard = match create_keyboard() {
            Ok(device) => {
                tracing::info!("uinput virtual keyboard created");
                Some(device)
            }
            Err(e) => {
                tracing::warn!("{e}");
                None
            }
        };
        Self {
            keyboard: Mutex::new(keyboard),
        }
    }

    fn hotkey(&self, hotkey: &crate::Hotkey) -> Result<(), ActionError> {
        let mut guard = self.keyboard.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            *guard = Some(create_keyboard()?);
            tracing::info!("uinput virtual keyboard created");
        }
        let Some(device) = guard.as_mut() else {
            unreachable!("keyboard was just created");
        };
        run_hotkey(&mut Uinput(device), hotkey, HOTKEY_HOLD)
    }
}

impl Default for LinuxExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for LinuxExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ready = self
            .keyboard
            .lock()
            .map(|k| k.is_some())
            .unwrap_or_default();
        f.debug_struct("LinuxExecutor")
            .field("keyboard_ready", &ready)
            .finish()
    }
}

impl ActionExecutor for LinuxExecutor {
    fn execute(&self, action: &Action) -> Result<(), ActionError> {
        let result = match action {
            Action::Hotkey(hotkey) => self.hotkey(hotkey),
        };
        log_outcome(action, &result);
        result
    }
}

fn create_keyboard() -> Result<VirtualDevice, ActionError> {
    let build = || -> io::Result<VirtualDevice> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for key in Key::ALL {
            keys.insert(KeyCode(key.evdev_code()));
        }
        VirtualDevice::builder()?
            .name(DEVICE_NAME)
            .with_keys(&keys)?
            .build()
    };
    let device = build().map_err(|e| match e.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound => {
            ActionError::Unavailable(format!(
                "cannot open /dev/uinput ({e}): keyboard shortcuts are disabled. \
                 Install the udev rule described in the README (section \"Linux: uinput access\")."
            ))
        }
        _ => ActionError::Failed(format!("cannot create the uinput virtual keyboard: {e}")),
    })?;
    thread::sleep(DEVICE_SETTLE);
    Ok(device)
}

struct Uinput<'a>(&'a mut VirtualDevice);

impl Uinput<'_> {
    fn emit(&mut self, key: Key, value: i32) -> Result<(), ActionError> {
        let event = InputEvent::new(EventType::KEY.0, key.evdev_code(), value);
        self.0
            .emit(&[event])
            .map_err(|e| ActionError::Failed(format!("uinput write failed for {key}: {e}")))
    }
}

impl KeyInjector for Uinput<'_> {
    fn key_down(&mut self, key: Key) -> Result<(), ActionError> {
        self.emit(key, 1)
    }

    fn key_up(&mut self, key: Key) -> Result<(), ActionError> {
        self.emit(key, 0)
    }
}
