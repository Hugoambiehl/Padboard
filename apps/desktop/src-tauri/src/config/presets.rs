//! The action library of the editor: ready-made shortcuts, adapted to the
//! operating system the desktop runs on (Windows or Linux). Their labels and
//! descriptions are UI text, translated by the frontend from the preset id.
//! Only actions that work are listed; the library grows with phase 4.

use padboard_actions::{Action, Hotkey, Key};
use padboard_protocol::Category;
use serde::Serialize;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PresetGroup {
    System,
    Media,
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
    Preset {
        id: id.to_owned(),
        group,
        color,
        icon: icon.to_owned(),
        action: Action::Hotkey(Hotkey::new(keys)),
    }
}

/// The library for this operating system.
#[must_use]
pub fn presets() -> Vec<Preset> {
    use Category::{Blue, Grey, Orange, Red, Teal, Violet};
    use Key::{
        AltLeft, AudioVolumeDown, AudioVolumeMute, AudioVolumeUp, ControlLeft, KeyC, KeyD, KeyL,
        KeyV, KeyX, KeyZ, MediaPlayPause, MediaStop, MediaTrackNext, MediaTrackPrevious, MetaLeft,
        ShiftLeft,
    };
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
        preset("lock", System, Red, "Lock", &[MetaLeft, KeyL]),
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
        preset("playPause", Media, Orange, "Play", &[MediaPlayPause]),
        preset(
            "previousTrack",
            Media,
            Orange,
            "SkipBack",
            &[MediaTrackPrevious],
        ),
        preset("nextTrack", Media, Orange, "SkipForward", &[MediaTrackNext]),
        preset("stop", Media, Orange, "Square", &[MediaStop]),
        preset("volumeUp", Media, Violet, "Volume2", &[AudioVolumeUp]),
        preset("volumeDown", Media, Violet, "Volume1", &[AudioVolumeDown]),
        preset("mute", Media, Violet, "VolumeX", &[AudioVolumeMute]),
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
            let Action::Hotkey(hotkey) = &p.action else {
                panic!("{} is not a hotkey", p.id);
            };
            if p.id != "hotkey" {
                hotkey.validate().unwrap();
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
        assert_eq!(keys("lock"), [Key::MetaLeft, Key::KeyL]);
    }
}
