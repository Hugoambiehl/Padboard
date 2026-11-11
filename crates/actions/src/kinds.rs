//! Parameters of each kind of [`Action`](crate::Action) other than the hotkey.
//!
//! Only "system" actions (hotkey, media, volume, microphone, open, lock) are
//! run by an [`ActionExecutor`](crate::ActionExecutor). Navigation, multi
//! actions and OBS are carried out by the desktop app, which owns the
//! sessions, the timing and the OBS connection.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// Media player command. Sent to the active player (MPRIS on Linux, the
/// system media session on Windows), with the media keys as a fallback.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAction {
    pub command: MediaCommand,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
    Stop,
}

/// Output volume of the default device.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeAction {
    pub command: VolumeCommand,
    /// Percent: the step for `up` / `down` (1–100), the level for `set` (0–100).
    /// Ignored by `toggle_mute`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<u32>,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeCommand {
    Up,
    Down,
    Set,
    ToggleMute,
}

/// Default step of `volume up` / `volume down`, in percent.
pub const DEFAULT_VOLUME_STEP: u32 = 5;

/// Default input device (microphone).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneAction {
    pub command: MicrophoneCommand,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicrophoneCommand {
    ToggleMute,
    Mute,
    Unmute,
}

/// Opens a URL (`https://…`), a file, a folder or a program with the
/// system's default handler.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAction {
    pub target: String,
}

/// Longest `open` target, in bytes.
pub const MAX_OPEN_TARGET: usize = 2048;

/// Moves the tablet that pressed the button (desktop app).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateAction {
    pub to: NavigateTo,
    /// Page or folder id, for `page` and `folder`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigateTo {
    Page,
    Folder,
    Next,
    Previous,
    Back,
}

/// Several actions in a row (desktop app). Steps cannot be multi actions.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiAction {
    pub steps: Vec<MultiStep>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStep {
    pub action: crate::Action,
    /// Pause after this step, in milliseconds.
    #[serde(default)]
    pub delay_ms: u32,
}

/// Most steps in a multi action.
pub const MAX_MULTI_STEPS: usize = 20;
/// Longest pause between two steps, in milliseconds.
pub const MAX_MULTI_DELAY_MS: u32 = 10_000;

/// OBS Studio, through obs-websocket v5 (desktop app).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsAction {
    pub command: ObsCommand,
    /// Scene name for `set_scene`, input (source) name for `toggle_input_mute`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsCommand {
    SetScene,
    ToggleRecord,
    ToggleStream,
    ToggleInputMute,
}

impl ObsCommand {
    /// Whether the command needs a `target` (scene or input name).
    #[must_use]
    pub const fn needs_target(self) -> bool {
        matches!(self, Self::SetScene | Self::ToggleInputMute)
    }
}

/// Longest scene, input, page or folder name referenced by an action.
pub const MAX_ACTION_NAME: usize = 256;
