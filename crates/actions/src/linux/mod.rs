//! Linux implementation.
//!
//! - Keyboard: uinput (below).
//! - Volume and microphone: PulseAudio protocol, served by PipeWire too ([`pulse`]).
//! - Media: MPRIS players ([`mpris`]), media keys as a fallback.
//! - Open and lock: `xdg-open` / `gio launch` / direct start, `loginctl` ([`launch`]).
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
    Action, ActionError, ActionExecutor, HOTKEY_HOLD, Hotkey, Key, KeyInjector, MediaAction,
    MediaCommand, MicrophoneAction, MicrophoneCommand, VolumeAction, VolumeCommand, log_outcome,
    not_yet, run_hotkey,
};

pub mod launch;
pub mod mpris;
pub mod pulse;

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

impl LinuxExecutor {
    /// MPRIS when a player is there, else the media key (some players only
    /// listen to keys).
    fn media(&self, media: &MediaAction) -> Result<(), ActionError> {
        match mpris::command(media.command) {
            Ok(true) => Ok(()),
            Ok(false) => self.media_key(media.command),
            Err(e) => {
                tracing::warn!("{e}, falling back to the media key");
                self.media_key(media.command)
            }
        }
    }

    fn media_key(&self, command: MediaCommand) -> Result<(), ActionError> {
        let key = match command {
            MediaCommand::PlayPause => Key::MediaPlayPause,
            MediaCommand::Next => Key::MediaTrackNext,
            MediaCommand::Previous => Key::MediaTrackPrevious,
            MediaCommand::Stop => Key::MediaStop,
        };
        self.hotkey(&Hotkey::new([key]))
    }
}

fn volume_action(volume: &VolumeAction) -> Result<(), ActionError> {
    let mut pulse = pulse::Pulse::connect()?;
    let sink = pulse.default_sink()?;
    match crate::audio::adjusted(
        &pulse::channels(&sink.cvolume),
        volume.command,
        volume.value,
    ) {
        None => pulse.set_sink_mute(sink.name, !sink.muted),
        Some(channels) => {
            // Changing the volume unmutes, as desktop volume keys do.
            if sink.muted && volume.command != VolumeCommand::Down {
                pulse.set_sink_mute(sink.name.clone(), false)?;
            }
            pulse.set_sink_volume(sink.name, &channels)
        }
    }
}

fn microphone(mic: &MicrophoneAction) -> Result<(), ActionError> {
    let mut pulse = pulse::Pulse::connect()?;
    let source = pulse.default_source()?;
    let mute = match mic.command {
        MicrophoneCommand::ToggleMute => !source.muted,
        MicrophoneCommand::Mute => true,
        MicrophoneCommand::Unmute => false,
    };
    pulse.set_source_mute(source.name, mute)
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
            Action::Hotkey(hotkey) => action.validate().and_then(|()| self.hotkey(hotkey)),
            Action::Media(media) => self.media(media),
            Action::Volume(volume) => action.validate().and_then(|()| volume_action(volume)),
            Action::Microphone(mic) => microphone(mic),
            Action::Open(open) => action.validate().and_then(|()| launch::open(&open.target)),
            Action::Lock => launch::lock(),
            other => not_yet(other),
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
