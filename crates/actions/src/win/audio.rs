//! Output volume and microphone through the Core Audio endpoint API
//! (`IAudioEndpointVolume`) of the default playback and recording devices.
//! The volume is the master scalar, the value of the Windows volume slider.

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};

use crate::state::AudioState;
use crate::{ActionError, MicrophoneCommand, VolumeAction, VolumeCommand};

/// COM for the current thread, released on drop. Executors run on tokio's
/// blocking threads, which COM may or may not have seen before.
struct Com {
    owned: bool,
}

impl Com {
    fn init() -> Result<Self, ActionError> {
        // SAFETY: plain COM initialization; S_FALSE (already initialized)
        // also counts as success and must be balanced too.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self { owned: true })
        } else if hr == windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            // Already initialized in another mode by someone else: usable.
            Ok(Self { owned: false })
        } else {
            Err(ActionError::Unavailable(format!("COM: {hr:?}")))
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
    }
}

/// `None` when there is no default device for that direction.
fn endpoint(flow: EDataFlow) -> Result<Option<IAudioEndpointVolume>, ActionError> {
    // SAFETY: COM is initialized on this thread by the caller (`Com`).
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| ActionError::Unavailable(format!("audio devices: {e}")))?;
        let Ok(device) = devices.GetDefaultAudioEndpoint(flow, eConsole) else {
            return Ok(None);
        };
        device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .map(Some)
            .map_err(|e| ActionError::Failed(format!("audio endpoint: {e}")))
    }
}

fn failed(e: windows::core::Error) -> ActionError {
    ActionError::Failed(format!("audio: {e}"))
}

pub fn volume(action: &VolumeAction) -> Result<(), ActionError> {
    let _com = Com::init()?;
    let output =
        endpoint(eRender)?.ok_or_else(|| ActionError::Unavailable("no output device".into()))?;
    // SAFETY: `output` is a live endpoint, COM initialized above.
    unsafe {
        let muted = output.GetMute().map_err(failed)?.as_bool();
        let level = f64::from(output.GetMasterVolumeLevelScalar().map_err(failed)?);
        match crate::audio::adjusted(&[level], action.command, action.value) {
            None => output.SetMute(!muted, std::ptr::null()).map_err(failed),
            Some(channels) => {
                if muted && action.command != VolumeCommand::Down {
                    output.SetMute(false, std::ptr::null()).map_err(failed)?;
                }
                // 0–1 by construction (`audio::adjusted`).
                #[allow(clippy::cast_possible_truncation)]
                let scalar = channels[0] as f32;
                output
                    .SetMasterVolumeLevelScalar(scalar, std::ptr::null())
                    .map_err(failed)
            }
        }
    }
}

pub fn microphone(command: MicrophoneCommand) -> Result<(), ActionError> {
    let _com = Com::init()?;
    let input =
        endpoint(eCapture)?.ok_or_else(|| ActionError::Unavailable("no microphone".into()))?;
    // SAFETY: `input` is a live endpoint, COM initialized above.
    unsafe {
        let muted = input.GetMute().map_err(failed)?.as_bool();
        let mute = match command {
            MicrophoneCommand::ToggleMute => !muted,
            MicrophoneCommand::Mute => true,
            MicrophoneCommand::Unmute => false,
        };
        input.SetMute(mute, std::ptr::null()).map_err(failed)
    }
}

pub fn state() -> Result<AudioState, ActionError> {
    let _com = Com::init()?;
    let output =
        endpoint(eRender)?.ok_or_else(|| ActionError::Unavailable("no output device".into()))?;
    let input = endpoint(eCapture)?;
    // SAFETY: live endpoints, COM initialized above.
    unsafe {
        let level = f64::from(output.GetMasterVolumeLevelScalar().map_err(failed)?);
        Ok(AudioState {
            output_percent: crate::audio::percent(&[level]),
            output_muted: output.GetMute().map_err(failed)?.as_bool(),
            input_muted: match input {
                Some(input) => Some(input.GetMute().map_err(failed)?.as_bool()),
                None => None,
            },
        })
    }
}
