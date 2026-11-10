//! Phase 1 demo profile, hard-coded until configuration persistence exists
//! (phase 3). Its labels are *user content* (what a user would type in the
//! editor), not UI strings, which is why they are not in the i18n resources.
//! Only hotkeys are available in phase 1; media and volume buttons use the
//! media keys, which work on both Linux and Windows.

use std::collections::HashMap;

use padboard_actions::{Action, Hotkey, Key};
use padboard_protocol::{ButtonView, Category, GridSize, Layout, Page, ProfileInfo};

/// What the server needs: the layout sent to tablets and the action bound to
/// each button id.
#[derive(Debug, Clone)]
pub struct Profile {
    pub layout: Layout,
    pub actions: HashMap<String, Action>,
}

struct DemoButton {
    id: &'static str,
    row: u32,
    col: u32,
    label: &'static str,
    subtitle: &'static str,
    category: Category,
    keys: &'static [Key],
}

const fn b(
    id: &'static str,
    (row, col): (u32, u32),
    label: &'static str,
    subtitle: &'static str,
    category: Category,
    keys: &'static [Key],
) -> DemoButton {
    DemoButton {
        id,
        row,
        col,
        label,
        subtitle,
        category,
        keys,
    }
}

#[rustfmt::skip]
const BUTTONS: &[DemoButton] = &[
    b("terminal", (0, 0), "Terminal", "Ctrl + Alt + T", Category::Grey, &[Key::ControlLeft, Key::AltLeft, Key::KeyT]),
    b("copy", (0, 1), "Copier", "Ctrl + C", Category::Blue, &[Key::ControlLeft, Key::KeyC]),
    b("paste", (0, 2), "Coller", "Ctrl + V", Category::Blue, &[Key::ControlLeft, Key::KeyV]),
    b("undo", (0, 3), "Annuler", "Ctrl + Z", Category::Teal, &[Key::ControlLeft, Key::KeyZ]),
    b("screenshot", (0, 4), "Capture", "Impr. écran", Category::Teal, &[Key::PrintScreen]),
    b("play-pause", (1, 0), "Lecture / Pause", "Médias", Category::Orange, &[Key::MediaPlayPause]),
    b("previous", (1, 1), "Piste précédente", "Médias", Category::Orange, &[Key::MediaTrackPrevious]),
    b("next", (1, 2), "Piste suivante", "Médias", Category::Orange, &[Key::MediaTrackNext]),
    b("volume-down", (1, 3), "Volume −", "Son PC", Category::Violet, &[Key::AudioVolumeDown]),
    b("volume-up", (1, 4), "Volume +", "Son PC", Category::Violet, &[Key::AudioVolumeUp]),
    b("mute", (2, 0), "Muet", "Son PC", Category::Violet, &[Key::AudioVolumeMute]),
    b("discord", (2, 1), "Discord", "F13 · raccourci global", Category::Pink, &[Key::F13]),
    b("desktop", (2, 2), "Bureau", "Super + D", Category::Grey, &[Key::MetaLeft, Key::KeyD]),
    b("lock", (2, 4), "Verrouiller", "Super + L", Category::Red, &[Key::MetaLeft, Key::KeyL]),
];

#[must_use]
pub fn profile() -> Profile {
    let buttons = BUTTONS
        .iter()
        .map(|d| ButtonView {
            id: d.id.to_owned(),
            row: d.row,
            col: d.col,
            label: d.label.to_owned(),
            subtitle: Some(d.subtitle.to_owned()),
            category: d.category,
        })
        .collect();
    let actions = BUTTONS
        .iter()
        .map(|d| (d.id.to_owned(), Action::Hotkey(Hotkey::new(d.keys))))
        .collect();
    Profile {
        layout: Layout {
            profile: ProfileInfo {
                id: "demo".to_owned(),
                name: "Démo".to_owned(),
            },
            grid: GridSize { cols: 5, rows: 3 },
            pages: vec![Page {
                id: "page-1".to_owned(),
                buttons,
            }],
        },
        actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_profile_is_consistent() {
        let p = profile();
        p.layout.validate().unwrap();
        let ids: Vec<_> = p.layout.pages[0].buttons.iter().map(|b| &b.id).collect();
        assert_eq!(ids.len(), p.actions.len());
        for id in ids {
            let Some(Action::Hotkey(hk)) = p.actions.get(id) else {
                panic!("no action for {id}");
            };
            hk.validate().unwrap();
        }
    }
}
