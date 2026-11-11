//! What the desktop app observes to show button states (microphone muted,
//! volume, media playing). Blocking, like the executors.

use crate::ActionError;

/// Output and microphone state of the default devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioState {
    /// Output volume, 0–100 %.
    pub output_percent: u32,
    pub output_muted: bool,
    /// `None` when there is no microphone.
    pub input_muted: Option<bool>,
}

/// Current audio state.
///
/// # Errors
/// [`ActionError::Unavailable`] without a sound server or output device.
pub fn audio() -> Result<AudioState, ActionError> {
    #[cfg(target_os = "linux")]
    {
        crate::linux::pulse::Pulse::connect()?.state()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(unsupported("audio state"))
    }
}

/// A feed of audio changes: [`AudioChanges::wait`] blocks until something
/// changes (volume, mute, default device), then read [`audio`] again.
pub struct AudioChanges {
    #[cfg(target_os = "linux")]
    inner: crate::linux::pulse::Subscription,
}

impl std::fmt::Debug for AudioChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AudioChanges")
    }
}

impl AudioChanges {
    /// Subscribes to audio changes.
    ///
    /// # Errors
    /// [`ActionError::Unavailable`] without a sound server.
    pub fn subscribe() -> Result<Self, ActionError> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                inner: crate::linux::pulse::Pulse::connect()?.subscribe()?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(unsupported("audio changes"))
        }
    }

    /// Blocks until the next change.
    ///
    /// # Errors
    /// When the sound server goes away: subscribe again later.
    pub fn wait(&mut self) -> Result<(), ActionError> {
        #[cfg(target_os = "linux")]
        {
            self.inner.next_change()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(unsupported("audio changes"))
        }
    }
}

/// Whether a media player is playing (`None`: no player).
///
/// # Errors
/// If the player list cannot be read.
pub fn media_playing() -> Result<Option<bool>, ActionError> {
    #[cfg(target_os = "linux")]
    {
        crate::linux::mpris::playing()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(unsupported("media state"))
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported(what: &'static str) -> ActionError {
    ActionError::Unsupported {
        action: what,
        platform: std::env::consts::OS,
    }
}
