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
    #[cfg(windows)]
    {
        crate::win::audio::state()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err(unsupported("audio state"))
    }
}

/// A feed of audio changes: [`AudioChanges::wait`] blocks until something
/// changes (volume, mute, default device), then read [`audio`] again.
pub struct AudioChanges {
    #[cfg(target_os = "linux")]
    inner: crate::linux::pulse::Subscription,
    /// Windows has no simple change feed: the state is polled and only real
    /// changes are reported.
    #[cfg(windows)]
    last: Option<AudioState>,
}

/// Polling interval of the Windows audio state.
#[cfg(windows)]
const POLL: std::time::Duration = std::time::Duration::from_millis(300);

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
        #[cfg(windows)]
        {
            Ok(Self {
                last: Some(audio()?),
            })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
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
        #[cfg(windows)]
        {
            loop {
                std::thread::sleep(POLL);
                let now = audio()?;
                if self.last != Some(now) {
                    self.last = Some(now);
                    return Ok(());
                }
            }
        }
        #[cfg(not(any(target_os = "linux", windows)))]
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
    #[cfg(windows)]
    {
        crate::win::media::playing()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err(unsupported("media state"))
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn unsupported(what: &'static str) -> ActionError {
    ActionError::Unsupported {
        action: what,
        platform: std::env::consts::OS,
    }
}
