//! The action library of the editor: ready-made shortcuts, adapted to the
//! operating system the desktop runs on (Windows or Linux). Their labels and
//! descriptions are UI text, translated by the frontend from the preset id.
//! Only actions that work are listed; the library grows with phase 4.

use padboard_actions::{
    Action, Hotkey, Key, MediaAction, MediaCommand, MicrophoneAction, MicrophoneCommand,
    NavigateAction, NavigateTo, VolumeAction, VolumeCommand,
};
use padboard_protocol::Category;
use serde::Serialize;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PresetGroup {
    System,
    Media,
    Navigation,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Stable id; also the i18n key of the label and description.
    pub id: String,
    pub group: PresetGroup,
    pub color: Category,
    /// Lucide icon name (the frontend resolves its paths).
    pub icon: String,
    pub action: Action,
}

fn preset(id: &str, group: PresetGroup, color: Category, icon: &str, keys: &[Key]) -> Preset {
    with_action(id, group, color, icon, Action::Hotkey(Hotkey::new(keys)))
}

fn with_action(
    id: &str,
    group: PresetGroup,
    color: Category,
    icon: &str,
    action: Action,
) -> Preset {
    Preset {
        id: id.to_owned(),
        group,
        color,
        icon: icon.to_owned(),
        action,
    }
}

fn media(id: &str, icon: &str, command: MediaCommand) -> Preset {
    with_action(
        id,
        PresetGroup::Media,
        Category::Orange,
        icon,
        Action::Media(MediaAction { command }),
    )
}

fn navigate(id: &str, icon: &str, to: NavigateTo) -> Preset {
    with_action(
        id,
        PresetGroup::Navigation,
        Category::Violet,
        icon,
        Action::Navigate(NavigateAction { to, id: None }),
    )
}

fn volume(id: &str, icon: &str, command: VolumeCommand) -> Preset {
    with_action(
        id,
        PresetGroup::Media,
        Category::Violet,
        icon,
        Action::Volume(VolumeAction {
            command,
            value: None,
        }),
    )
}

/// The library for this operating system.
#[must_use]
pub fn presets() -> Vec<Preset> {
    use Category::{Blue, Grey, Red, Teal, Violet};
    use Key::{AltLeft, ControlLeft, KeyC, KeyD, KeyV, KeyX, KeyZ, MetaLeft, ShiftLeft};
    use PresetGroup::{Media, System};

    let windows = cfg!(windows);
    let redo: &[Key] = if windows {
        &[ControlLeft, Key::KeyY]
    } else {
        &[ControlLeft, ShiftLeft, KeyZ]
    };
    let screenshot: &[Key] = if windows {
        &[MetaLeft, ShiftLeft, Key::KeyS]
    } else {
        &[Key::PrintScreen]
    };

    let mut list = vec![
        // An empty shortcut, recorded in the inspector.
        preset("hotkey", System, Grey, "Keyboard", &[]),
        preset("copy", System, Blue, "Copy", &[ControlLeft, KeyC]),
        preset("cut", System, Blue, "Scissors", &[ControlLeft, KeyX]),
        preset(
            "paste",
            System,
            Blue,
            "ClipboardPaste",
            &[ControlLeft, KeyV],
        ),
        preset("undo", System, Teal, "Undo2", &[ControlLeft, KeyZ]),
        preset("redo", System, Teal, "Redo2", redo),
        preset("screenshot", System, Teal, "Camera", screenshot),
        preset("showDesktop", System, Grey, "Monitor", &[MetaLeft, KeyD]),
        with_action("lock", System, Red, "Lock", Action::Lock),
    ];
    if windows {
        list.push(preset(
            "taskManager",
            System,
            Grey,
            "Activity",
            &[ControlLeft, ShiftLeft, Key::Escape],
        ));
    } else {
        list.push(preset(
            "terminal",
            System,
            Grey,
            "Terminal",
            &[ControlLeft, AltLeft, Key::KeyT],
        ));
    }
    list.extend([
        // The mockup's main button: microphone on/off, its state shown.
        with_action(
            "microphone",
            Media,
            Violet,
            "Mic",
            Action::Microphone(MicrophoneAction {
                command: MicrophoneCommand::ToggleMute,
            }),
        ),
        volume("volumeUp", "Volume2", VolumeCommand::Up),
        volume("volumeDown", "Volume1", VolumeCommand::Down),
        volume("mute", "VolumeX", VolumeCommand::ToggleMute),
        media("playPause", "Play", MediaCommand::PlayPause),
        media("previousTrack", "SkipBack", MediaCommand::Previous),
        media("nextTrack", "SkipForward", MediaCommand::Next),
        media("stop", "Square", MediaCommand::Stop),
        // Creates a folder and its button (`AddFolder`), see the frontend.
        navigate("folder", "Folder", NavigateTo::Folder),
        navigate("nextPage", "ChevronRight", NavigateTo::Next),
        navigate("previousPage", "ChevronLeft", NavigateTo::Previous),
        navigate("back", "Undo2", NavigateTo::Back),
    ]);
    list
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn presets_are_valid_and_unique() {
        let list = presets();
        let ids: HashSet<_> = list.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), list.len());
        for p in &list {
            // Drafts: the empty shortcut (recorded in the inspector), the
            // folder (created with the button).
            if p.id == "hotkey" || p.id == "folder" {
                p.action.validate_draft().unwrap();
            } else {
                p.action.validate().unwrap();
            }
        }
    }

    #[test]
    fn shortcuts_follow_the_operating_system() {
        let keys = |id: &str| {
            let p = presets().into_iter().find(|p| p.id == id).unwrap();
            let Action::Hotkey(h) = p.action else {
                unreachable!()
            };
            h.keys
        };
        if cfg!(windows) {
            assert_eq!(
                keys("screenshot"),
                [Key::MetaLeft, Key::ShiftLeft, Key::KeyS]
            );
        } else {
            assert_eq!(keys("screenshot"), [Key::PrintScreen]);
        }
        let lock = presets().into_iter().find(|p| p.id == "lock").unwrap();
        assert_eq!(lock.action, Action::Lock);
    }
}
