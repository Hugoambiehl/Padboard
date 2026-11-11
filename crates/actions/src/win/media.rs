//! Media players through the system media session (the "now playing"
//! overlay of Windows): Spotify, browsers, the Media Player…

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};

use crate::{ActionError, MediaCommand};

fn failed(e: windows::core::Error) -> ActionError {
    ActionError::Failed(format!("media session: {e}"))
}

fn current() -> Result<Option<GlobalSystemMediaTransportControlsSession>, ActionError> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|op| op.join())
        .map_err(failed)?;
    Ok(manager.GetCurrentSession().ok())
}

/// Sends `command` to the current session. `Ok(false)` when there is none
/// (the caller then presses the media key).
pub fn command(command: MediaCommand) -> Result<bool, ActionError> {
    let Some(session) = current()? else {
        return Ok(false);
    };
    let accepted = match command {
        MediaCommand::PlayPause => session.TryTogglePlayPauseAsync(),
        MediaCommand::Next => session.TrySkipNextAsync(),
        MediaCommand::Previous => session.TrySkipPreviousAsync(),
        MediaCommand::Stop => session.TryStopAsync(),
    }
    .and_then(|op| op.join())
    .map_err(failed)?;
    // Refused by the player (e.g. no "previous"): let the key try.
    Ok(accepted)
}

/// Whether the current session is playing (`None`: no session).
pub fn playing() -> Result<Option<bool>, ActionError> {
    let Some(session) = current()? else {
        return Ok(None);
    };
    let status = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map_err(failed)?;
    Ok(Some(status == Status::Playing))
}
