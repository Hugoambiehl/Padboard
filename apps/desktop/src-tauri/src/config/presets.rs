//! The action library of the editor: ready-made shortcuts, adapted to the
//! operating system the desktop runs on (Windows or Linux). Their labels and
//! descriptions are UI text, translated by the frontend from the preset id.
//! Only actions that work are listed; the library grows with phase 4.

use padboard_actions::{
    Action, Hotkey, Key, MediaAction, MediaCommand, MicrophoneAction, MicrophoneCommand,
    MultiAction, NavigateAction, NavigateTo, ObsAction, ObsCommand, OpenAction, VolumeAction,
    VolumeCommand,
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
    Obs,
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

fn obs(id: &str, color: Category, icon: &str, command: ObsCommand) -> Preset {
    with_action(
        id,
        PresetGroup::Obs,
        color,
        icon,
        Action::Obs(ObsAction {
            command,
            target: None,
        }),
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
pub fn presets(second_screen: bool) -> Vec<Preset> {
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
        // Three uses of the same `open` action, each one obvious in the
        // library ("Documents" alone hid that it can start a program).
        with_action(
            "launchApp",
            System,
            Violet,
            "AppWindow",
            // Chosen in the inspector ("Application…").
            Action::Open(OpenAction {
                target: String::new(),
            }),
        ),
        with_action(
            "openPath",
            System,
            Blue,
            "FolderOpen",
            // An example that works at once: the Documents folder.
            Action::Open(OpenAction {
                target: documents_folder(),
            }),
        ),
        with_action(
            "website",
            System,
            Teal,
            "Globe",
            // Typed in the inspector.
            Action::Open(OpenAction {
                target: String::new(),
            }),
        ),
        // Steps added in the inspector.
        with_action(
            "multi",
            System,
            Violet,
            "ListChecks",
            Action::Multi(MultiAction { steps: Vec::new() }),
        ),
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
    if second_screen {
        // Shows the PC on the tablet that pressed it (#20).
        list.push(with_action(
            "secondScreen",
            System,
            Blue,
            "MonitorSmartphone",
            Action::SecondScreen,
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
        // Scene and source chosen in the inspector.
        obs(
            "obsScene",
            Category::Teal,
            "Clapperboard",
            ObsCommand::SetScene,
        ),
        obs(
            "obsRecord",
            Category::Red,
            "Circle",
            ObsCommand::ToggleRecord,
        ),
        obs(
            "obsStream",
            Category::Pink,
            "Radio",
            ObsCommand::ToggleStream,
        ),
        obs(
            "obsMute",
            Category::Violet,
            "Mic",
            ObsCommand::ToggleInputMute,
        ),
    ]);
    list
}

/// The user's Documents folder, as the system knows it (it may be
/// redirected or translated, so never a path built by hand).
#[cfg(windows)]
fn documents_folder() -> String {
    // The shell's name of the Documents known folder (FOLDERID_Documents):
    // `open` hands it to ShellExecute, which resolves it, redirection
    // (OneDrive, another drive) included.
    "shell:Personal".to_owned()
}

/// The user's Documents folder, as the system knows it (XDG user
/// directory, translated on some systems), `~/` for the home folder.
#[cfg(not(windows))]
fn documents_folder() -> String {
    let resolved = std::process::Command::new("xdg-user-dir")
        .arg("DOCUMENTS")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|dir| dir.trim().to_owned());
    let home = std::env::var("HOME").ok();
    match (resolved, home) {
        // xdg-user-dir answers the home folder itself when unset.
        (Some(dir), Some(home)) if dir != home.trim_end_matches('/') => {
            match dir.strip_prefix(&format!("{}/", home.trim_end_matches('/'))) {
                Some(rest) => format!("~/{rest}"),
                None => dir,
            }
        }
        _ => "~/Documents".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn presets_are_valid_and_unique() {
        let list = presets(true);
        let ids: HashSet<_> = list.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), list.len());
        for p in &list {
            // Drafts: the empty shortcut (recorded in the inspector), the
            // folder (created with the button), the application and the
            // site (chosen or typed in the inspector).
            if [
                "hotkey",
                "folder",
                "multi",
                "obsScene",
                "obsMute",
                "launchApp",
                "website",
            ]
            .contains(&p.id.as_str())
            {
                p.action.validate_draft().unwrap();
            } else {
                p.action.validate().unwrap();
            }
        }
    }

    #[test]
    fn the_second_screen_is_listed_only_with_a_source() {
        let has = |listed: bool| presets(listed).iter().any(|p| p.id == "secondScreen");
        assert!(has(true));
        assert!(!has(false));
    }

    #[test]
    fn shortcuts_follow_the_operating_system() {
        let keys = |id: &str| {
            let p = presets(true).into_iter().find(|p| p.id == id).unwrap();
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
        let lock = presets(true).into_iter().find(|p| p.id == "lock").unwrap();
        assert_eq!(lock.action, Action::Lock);
    }
}
