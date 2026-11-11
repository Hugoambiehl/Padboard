//! The configuration created at first launch: one "Démo" profile. Its labels
//! are *user content* (what a user would type in the editor), not UI strings,
//! which is why they are not in the i18n resources.
//!
//! Icons: Lucide (ISC license) path data, as `src/icons/paths.ts` converts it
//! (lucide 1.71.0).

use padboard_actions::{Action, Hotkey, Key};
use padboard_protocol::{Category, GridSize};

use super::model::{ButtonConfig, Config, IconConfig, PageConfig, ProfileConfig};

struct Seed {
    id: &'static str,
    cell: (u32, u32),
    label: &'static str,
    subtitle: &'static str,
    color: Category,
    keys: &'static [Key],
    icon: (&'static str, &'static [&'static str]),
}

#[cfg(not(windows))]
const TERMINAL: &[&str] = &["M12 19h8", "m4 17 6-6-6-6"];
#[cfg(windows)]
const FOLDER_OPEN: &[&str] = &[
    "m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2",
];
const COPY: &[&str] = &[
    "M10 8H20A2 2 0 0 1 22 10V20A2 2 0 0 1 20 22H10A2 2 0 0 1 8 20V10A2 2 0 0 1 10 8z",
    "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2",
];
const CLIPBOARD_PASTE: &[&str] = &[
    "M11 14h10",
    "M16 4h2a2 2 0 0 1 2 2v1.344",
    "m17 18 4-4-4-4",
    "M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 1.793-1.113",
    "M9 2H15A1 1 0 0 1 16 3V5A1 1 0 0 1 15 6H9A1 1 0 0 1 8 5V3A1 1 0 0 1 9 2z",
];
const UNDO: &[&str] = &[
    "M9 14 4 9l5-5",
    "M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11",
];
const CAMERA: &[&str] = &[
    "M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z",
    "M9 13a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z",
];
const PLAY: &[&str] =
    &["M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"];
const SKIP_BACK: &[&str] = &[
    "M17.971 4.285A2 2 0 0 1 21 6v12a2 2 0 0 1-3.029 1.715l-9.997-5.998a2 2 0 0 1-.003-3.432z",
    "M3 20V4",
];
const SKIP_FORWARD: &[&str] = &[
    "M21 4v16",
    "M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z",
];
const SPEAKER: &str = "M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z";
const VOLUME_1: &[&str] = &[SPEAKER, "M16 9a5 5 0 0 1 0 6"];
const VOLUME_2: &[&str] = &[
    SPEAKER,
    "M16 9a5 5 0 0 1 0 6",
    "M19.364 18.364a9 9 0 0 0 0-12.728",
];
const VOLUME_X: &[&str] = &[SPEAKER, "m16.5 14.5 5-5", "m16.5 9.5 5 5"];
const HEADPHONES: &[&str] = &[
    "M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3",
];
const MONITOR: &[&str] = &[
    "M4 3H20A2 2 0 0 1 22 5V15A2 2 0 0 1 20 17H4A2 2 0 0 1 2 15V5A2 2 0 0 1 4 3z",
    "M8 21L16 21",
    "M12 17L12 21",
];
const LOCK: &[&str] = &[
    "M5 11H19A2 2 0 0 1 21 13V20A2 2 0 0 1 19 22H5A2 2 0 0 1 3 20V13A2 2 0 0 1 5 11z",
    "M7 11V7a5 5 0 0 1 10 0v4",
];

/// First button: a terminal on Linux, the file explorer on Windows (no
/// standard terminal shortcut there).
#[cfg(not(windows))]
const FIRST: Seed = Seed {
    id: "terminal",
    cell: (0, 0),
    label: "Terminal",
    subtitle: "Ctrl + Alt + T",
    color: Category::Grey,
    keys: &[Key::ControlLeft, Key::AltLeft, Key::KeyT],
    icon: ("Terminal", TERMINAL),
};
#[cfg(windows)]
const FIRST: Seed = Seed {
    id: "explorer",
    cell: (0, 0),
    label: "Explorateur",
    subtitle: "Win + E",
    color: Category::Grey,
    keys: &[Key::MetaLeft, Key::KeyE],
    icon: ("FolderOpen", FOLDER_OPEN),
};

/// Screenshot: the snipping tool on Windows, Print Screen elsewhere.
#[cfg(not(windows))]
const SCREENSHOT: (&str, &[Key]) = ("Impr. écran", &[Key::PrintScreen]);
#[cfg(windows)]
const SCREENSHOT: (&str, &[Key]) = ("Win + Maj + S", &[Key::MetaLeft, Key::ShiftLeft, Key::KeyS]);

#[cfg(not(windows))]
const META: &str = "Super";
#[cfg(windows)]
const META: &str = "Win";

fn seeds() -> Vec<Seed> {
    let s = |id, cell, label, subtitle, color, keys, icon| Seed {
        id,
        cell,
        label,
        subtitle,
        color,
        keys,
        icon,
    };
    vec![
        FIRST,
        s(
            "copy",
            (0, 1),
            "Copier",
            "Ctrl + C",
            Category::Blue,
            &[Key::ControlLeft, Key::KeyC],
            ("Copy", COPY),
        ),
        s(
            "paste",
            (0, 2),
            "Coller",
            "Ctrl + V",
            Category::Blue,
            &[Key::ControlLeft, Key::KeyV],
            ("ClipboardPaste", CLIPBOARD_PASTE),
        ),
        s(
            "undo",
            (0, 3),
            "Annuler",
            "Ctrl + Z",
            Category::Teal,
            &[Key::ControlLeft, Key::KeyZ],
            ("Undo2", UNDO),
        ),
        s(
            "screenshot",
            (0, 4),
            "Capture",
            SCREENSHOT.0,
            Category::Teal,
            SCREENSHOT.1,
            ("Camera", CAMERA),
        ),
        s(
            "play-pause",
            (1, 0),
            "Lecture / Pause",
            "Médias",
            Category::Orange,
            &[Key::MediaPlayPause],
            ("Play", PLAY),
        ),
        s(
            "previous",
            (1, 1),
            "Piste précédente",
            "Médias",
            Category::Orange,
            &[Key::MediaTrackPrevious],
            ("SkipBack", SKIP_BACK),
        ),
        s(
            "next",
            (1, 2),
            "Piste suivante",
            "Médias",
            Category::Orange,
            &[Key::MediaTrackNext],
            ("SkipForward", SKIP_FORWARD),
        ),
        s(
            "volume-down",
            (1, 3),
            "Volume −",
            "Son PC",
            Category::Violet,
            &[Key::AudioVolumeDown],
            ("Volume1", VOLUME_1),
        ),
        s(
            "volume-up",
            (1, 4),
            "Volume +",
            "Son PC",
            Category::Violet,
            &[Key::AudioVolumeUp],
            ("Volume2", VOLUME_2),
        ),
        s(
            "mute",
            (2, 0),
            "Muet",
            "Son PC",
            Category::Violet,
            &[Key::AudioVolumeMute],
            ("VolumeX", VOLUME_X),
        ),
        s(
            "discord",
            (2, 1),
            "Discord",
            "F13 · raccourci global",
            Category::Pink,
            &[Key::F13],
            ("Headphones", HEADPHONES),
        ),
        s(
            "desktop",
            (2, 2),
            "Bureau",
            "",
            Category::Grey,
            &[Key::MetaLeft, Key::KeyD],
            ("Monitor", MONITOR),
        ),
        s(
            "lock",
            (2, 4),
            "Verrouiller",
            "",
            Category::Red,
            &[Key::MetaLeft, Key::KeyL],
            ("Lock", LOCK),
        ),
    ]
}

#[must_use]
pub fn config() -> Config {
    let buttons = seeds()
        .into_iter()
        .map(|s| ButtonConfig {
            id: s.id.to_owned(),
            row: s.cell.0,
            col: s.cell.1,
            label: s.label.to_owned(),
            subtitle: Some(match s.id {
                "desktop" => format!("{META} + D"),
                "lock" => format!("{META} + L"),
                _ => s.subtitle.to_owned(),
            }),
            icon: Some(IconConfig {
                name: s.icon.0.to_owned(),
                paths: s.icon.1.iter().map(|p| (*p).to_owned()).collect(),
            }),
            color: s.color,
            action: Action::Hotkey(Hotkey::new(s.keys)),
        })
        .collect();
    Config {
        active_profile_id: "demo".to_owned(),
        profiles: vec![ProfileConfig {
            id: "demo".to_owned(),
            name: "Démo".to_owned(),
            grid: GridSize { cols: 5, rows: 3 },
            folders: Vec::new(),
            pages: vec![PageConfig {
                id: "page-1".to_owned(),
                name: "Page 1".to_owned(),
                buttons,
            }],
        }],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_seed_is_valid() {
        super::config().validate().unwrap();
    }
}
