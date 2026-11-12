//! Editable configuration: model, invariants, first-launch content, and what
//! the server derives from it (the layout sent to tablets and the action of
//! each button).

pub mod edit;
mod model;
pub mod presets;
pub mod seed;
mod svg_path;
mod validate;

use std::collections::HashMap;

use padboard_actions::Action;
use padboard_protocol::{ButtonIcon, ButtonView, Folder, Layout, Page, ProfileInfo};

use crate::storage::{ConfigDir, Document};

pub use model::{ActiveLook, Config, PageConfig, ProfileConfig};
pub use validate::EditError;

impl Document for Config {
    const FILE: &'static str = "profiles.json";
    /// 2: folders (phase 4).
    const VERSION: u32 = 2;

    fn migrate(
        from: u32,
        mut fields: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        match from {
            1 => {
                let profiles = fields
                    .get_mut("profiles")
                    .and_then(serde_json::Value::as_array_mut)
                    .ok_or("no profiles array")?;
                for profile in profiles {
                    let profile = profile.as_object_mut().ok_or("profile is not an object")?;
                    profile
                        .entry("folders")
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                }
                Ok(fields)
            }
            _ => Err(format!("no migration from version {from}")),
        }
    }
}

/// Loads `profiles.json`, creating it with the [`seed`] at first launch. A
/// file that breaks the invariants (edited by hand…) is moved aside and
/// replaced by the seed, like an unreadable one.
#[must_use]
pub fn load(dir: &ConfigDir) -> Config {
    let config = dir.load_or_init(seed::config);
    match config.validate() {
        Ok(()) => config,
        Err(e) => {
            tracing::error!("{}: {e}", Config::FILE);
            dir.move_raw_aside(Config::FILE);
            let fresh = seed::config();
            if let Err(e) = dir.save(&fresh) {
                tracing::error!("cannot save {}: {e}", Config::FILE);
            }
            fresh
        }
    }
}

/// What the server needs from the active profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLayout {
    pub layout: Layout,
    pub actions: HashMap<String, Action>,
    /// Custom looks while active, by button.
    pub looks: HashMap<String, ActiveLook>,
}

impl Config {
    #[must_use]
    pub fn active_profile(&self) -> Option<&ProfileConfig> {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
    }

    /// The layout and actions of the active profile (empty if, against the
    /// invariants, there is none).
    #[must_use]
    pub fn active_layout(&self) -> ActiveLayout {
        let Some(profile) = self.active_profile().or_else(|| self.profiles.first()) else {
            return ActiveLayout {
                layout: Layout {
                    profile: ProfileInfo {
                        id: String::new(),
                        name: String::new(),
                    },
                    grid: padboard_protocol::GridSize { cols: 1, rows: 1 },
                    pages: Vec::new(),
                    folders: Vec::new(),
                },
                actions: HashMap::new(),
                looks: HashMap::new(),
            };
        };
        let buttons = |page: &PageConfig| {
            page.buttons
                .iter()
                .map(|b| ButtonView {
                    id: b.id.clone(),
                    row: b.row,
                    col: b.col,
                    label: b.label.clone(),
                    subtitle: b
                        .subtitle
                        .clone()
                        .filter(|s| !s.is_empty())
                        .or_else(|| folder_subtitle(profile, &b.action)),
                    category: b.color,
                    icon: b.icon.as_ref().map(|i| ButtonIcon {
                        paths: i.paths.clone(),
                    }),
                    active: false,
                    unavailable: false,
                    badge: None,
                })
                .collect()
        };
        ActiveLayout {
            layout: Layout {
                profile: ProfileInfo {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                },
                grid: profile.grid,
                pages: profile
                    .pages
                    .iter()
                    .map(|page| Page {
                        id: page.id.clone(),
                        buttons: buttons(page),
                    })
                    .collect(),
                folders: profile
                    .folders
                    .iter()
                    .map(|folder| Folder {
                        id: folder.id.clone(),
                        name: folder.name.clone(),
                        buttons: buttons(folder),
                    })
                    .collect(),
            },
            actions: profile
                .pages
                .iter()
                .chain(&profile.folders)
                .flat_map(|p| &p.buttons)
                .map(|b| (b.id.clone(), b.action.clone()))
                .collect(),
            looks: profile
                .pages
                .iter()
                .chain(&profile.folders)
                .flat_map(|p| &p.buttons)
                .filter_map(|b| b.active_look.clone().map(|l| (b.id.clone(), l)))
                .collect(),
        }
    }
}

/// "Dossier · 3 boutons" under a button opening a folder (Main.dc.html),
/// unless the user wrote a subtitle, in the PC's language (`locale.rs`).
fn folder_subtitle(profile: &ProfileConfig, action: &Action) -> Option<String> {
    let Action::Navigate(padboard_actions::NavigateAction {
        to: padboard_actions::NavigateTo::Folder,
        id: Some(id),
    }) = action
    else {
        return None;
    };
    let folder = profile.folders.iter().find(|f| &f.id == id)?;
    Some(crate::locale::texts().folder(folder.buttons.len()))
}

/// A fresh id for a profile, page or button (12 hex characters).
#[must_use]
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::validate::{MAX_COLS, MAX_TEXT};

    #[test]
    fn the_seed_gives_a_valid_layout_with_icons() {
        let active = seed::config().active_layout();
        active.layout.validate().unwrap();
        assert_eq!(active.layout.profile.name, "Démo");
        let buttons = &active.layout.pages[0].buttons;
        assert_eq!(buttons.len(), active.actions.len());
        assert!(buttons.iter().all(|b| b.icon.is_some()));
    }

    #[test]
    fn invalid_configurations_are_refused() {
        let base = seed::config();
        fn page(c: &mut Config) -> &mut PageConfig {
            &mut c.profiles[0].pages[0]
        }
        type Mutation = Box<dyn Fn(&mut Config)>;
        let cases: Vec<(&str, Mutation)> = vec![
            ("no profile", Box::new(|c| c.profiles.clear())),
            (
                "unknown active",
                Box::new(|c| c.active_profile_id = "nope".into()),
            ),
            ("no page", Box::new(|c| c.profiles[0].pages.clear())),
            (
                "grid too wide",
                Box::new(|c| c.profiles[0].grid.cols = MAX_COLS + 1),
            ),
            (
                "outside grid",
                Box::new(move |c| page(c).buttons[0].row = 9),
            ),
            (
                "same cell",
                Box::new(move |c| {
                    let p = page(c);
                    p.buttons[1].row = p.buttons[0].row;
                    p.buttons[1].col = p.buttons[0].col;
                }),
            ),
            (
                "duplicate id",
                Box::new(move |c| {
                    let p = page(c);
                    p.buttons[1].id = p.buttons[0].id.clone();
                }),
            ),
            (
                "bad id",
                Box::new(move |c| page(c).buttons[0].id = "a b".into()),
            ),
            (
                "long label",
                Box::new(move |c| page(c).buttons[0].label = "x".repeat(MAX_TEXT + 1)),
            ),
            (
                "empty page name",
                Box::new(move |c| page(c).name = "  ".into()),
            ),
            (
                "bad icon path",
                Box::new(move |c| {
                    page(c).buttons[0].icon.as_mut().unwrap().paths = vec!["M1 2 <x>".into()];
                }),
            ),
            (
                "too many icon paths",
                Box::new(move |c| {
                    page(c).buttons[0].icon.as_mut().unwrap().paths = vec!["M1 2".into(); 33];
                }),
            ),
            (
                "bad icon name",
                Box::new(move |c| {
                    page(c).buttons[0].icon.as_mut().unwrap().name = "../x".into();
                }),
            ),
            (
                "repeated key",
                Box::new(move |c| {
                    page(c).buttons[0].action = Action::Hotkey(padboard_actions::Hotkey::new([
                        padboard_actions::Key::KeyA,
                        padboard_actions::Key::KeyA,
                    ]));
                }),
            ),
        ];
        for (name, mutate) in cases {
            let mut c = base.clone();
            mutate(&mut c);
            assert!(c.validate().is_err(), "{name} should be refused");
        }
    }

    #[test]
    fn json_shape() {
        let json = serde_json::to_value(seed::config()).unwrap();
        let button = &json["profiles"][0]["pages"][0]["buttons"][1];
        assert_eq!(button["color"], "blue");
        assert_eq!(button["action"]["type"], "hotkey");
        assert_eq!(button["action"]["params"]["keys"][0], "ControlLeft");
        assert_eq!(button["icon"]["name"], "Copy");
    }

    #[test]
    fn ids_are_short_and_unique() {
        let a = new_id();
        assert_eq!(a.len(), 12);
        assert_ne!(a, new_id());
    }

    #[test]
    fn version_1_files_gain_empty_folders() {
        let dir = tempfile::tempdir().unwrap();
        let mut v1 = serde_json::to_value(seed::config()).unwrap();
        for profile in v1["profiles"].as_array_mut().unwrap() {
            profile.as_object_mut().unwrap().remove("folders");
        }
        v1["version"] = 1.into();
        std::fs::write(
            dir.path().join(Config::FILE),
            serde_json::to_vec(&v1).unwrap(),
        )
        .unwrap();
        let config = load(&ConfigDir::open(dir.path()).unwrap());
        assert_eq!(config.profiles[0].id, "demo");
        assert!(config.profiles[0].folders.is_empty());
    }
}
