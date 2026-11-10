//! Edit operations on the [`Config`] and the undo/redo history.
//!
//! The frontend never sends a whole configuration: it sends one [`Edit`] at a
//! time. Each one is applied to a copy, the copy is validated, and only then
//! does it replace the current configuration, so an invalid edit changes
//! nothing. History entries are full snapshots (the configuration is small).

use std::collections::VecDeque;

use padboard_actions::Action;
use padboard_protocol::{Category, GridSize};
use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use super::model::{ButtonConfig, Config, IconConfig, PageConfig, ProfileConfig};
use super::new_id;
use super::validate::{ConfigError, EditError};

/// Undo entries kept.
const HISTORY: usize = 100;

/// One change requested by the editor.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "camelCase")]
pub enum Edit {
    /// New profile with one empty page and a 5 × 3 grid.
    #[serde(rename_all = "camelCase")]
    AddProfile { name: String },
    /// Copy of a profile (new ids everywhere).
    #[serde(rename_all = "camelCase")]
    DuplicateProfile { profile_id: String, name: String },
    #[serde(rename_all = "camelCase")]
    RenameProfile { profile_id: String, name: String },
    /// Refused for the last profile. The active profile moves to the first one left.
    #[serde(rename_all = "camelCase")]
    DeleteProfile { profile_id: String },
    #[serde(rename_all = "camelCase")]
    SetActiveProfile { profile_id: String },
    /// Refused (`outsideGrid`) if buttons would fall outside.
    #[serde(rename_all = "camelCase")]
    SetGrid {
        profile_id: String,
        cols: u32,
        rows: u32,
    },
    #[serde(rename_all = "camelCase")]
    AddPage { profile_id: String, name: String },
    #[serde(rename_all = "camelCase")]
    RenamePage { page_id: String, name: String },
    /// Moves a page to `index` within its profile.
    #[serde(rename_all = "camelCase")]
    MovePage { page_id: String, index: u32 },
    /// Refused for the last page of a profile.
    #[serde(rename_all = "camelCase")]
    DeletePage { page_id: String },
    #[serde(rename_all = "camelCase")]
    AddButton {
        page_id: String,
        row: u32,
        col: u32,
        label: String,
        subtitle: Option<String>,
        icon: Option<IconConfig>,
        color: Category,
        action: Action,
    },
    /// Moves a button to a cell of a page of the same profile. If the cell is
    /// taken, the two buttons swap places.
    #[serde(rename_all = "camelCase")]
    MoveButton {
        button_id: String,
        page_id: String,
        row: u32,
        col: u32,
    },
    #[serde(rename_all = "camelCase")]
    SetLabel { button_id: String, label: String },
    #[serde(rename_all = "camelCase")]
    SetSubtitle {
        button_id: String,
        subtitle: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    SetIcon {
        button_id: String,
        icon: Option<IconConfig>,
    },
    #[serde(rename_all = "camelCase")]
    SetColor { button_id: String, color: Category },
    #[serde(rename_all = "camelCase")]
    SetAction { button_id: String, action: Action },
    #[serde(rename_all = "camelCase")]
    DeleteButton { button_id: String },
}

impl Edit {
    /// Successive edits with the same key (typing in one field) make a single
    /// undo entry.
    fn merge_key(&self) -> Option<String> {
        match self {
            Self::RenameProfile { profile_id, .. } => Some(format!("profile-name:{profile_id}")),
            Self::RenamePage { page_id, .. } => Some(format!("page-name:{page_id}")),
            Self::SetLabel { button_id, .. } => Some(format!("label:{button_id}")),
            Self::SetSubtitle { button_id, .. } => Some(format!("subtitle:{button_id}")),
            _ => None,
        }
    }
}

/// What an edit created, for the frontend to select it.
#[typeshare]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_id: Option<String>,
}

/// What the editor UI shows, sent on every change (`editor` event).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    pub config: Config,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// The configuration and its history.
#[derive(Debug)]
pub struct Editor {
    config: Config,
    undo: VecDeque<Config>,
    redo: Vec<Config>,
    /// Merge key of the last edit, while it can still absorb the next one.
    merging: Option<String>,
}

impl Editor {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            undo: VecDeque::new(),
            redo: Vec::new(),
            merging: None,
        }
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn state(&self) -> EditorState {
        EditorState {
            config: self.config.clone(),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Applies an edit if the result is valid.
    ///
    /// # Errors
    /// Why the edit was refused; the configuration is unchanged.
    pub fn apply(&mut self, edit: Edit) -> Result<EditOutcome, EditError> {
        let key = edit.merge_key();
        let mut next = self.config.clone();
        let outcome = apply(&mut next, edit)?;
        next.validate()?;
        if next == self.config {
            return Ok(outcome);
        }
        let merge = key.is_some() && key == self.merging;
        if !merge {
            self.undo
                .push_back(std::mem::replace(&mut self.config, next));
            if self.undo.len() > HISTORY {
                self.undo.pop_front();
            }
        } else {
            self.config = next;
        }
        self.redo.clear();
        self.merging = key;
        Ok(outcome)
    }

    /// Returns `false` if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.config, previous));
        self.merging = None;
        true
    }

    /// Returns `false` if there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push_back(std::mem::replace(&mut self.config, next));
        self.merging = None;
        true
    }

    /// Ends the current merge: the next edit of the same field gets its own
    /// undo entry (the user left the field).
    pub fn checkpoint(&mut self) {
        self.merging = None;
    }
}

fn not_found(what: &str, id: &str) -> EditError {
    EditError {
        code: "notFound".to_owned(),
        detail: format!("no {what} {id:?}"),
    }
}

fn refused(code: &str, detail: &str) -> EditError {
    EditError {
        code: code.to_owned(),
        detail: detail.to_owned(),
    }
}

fn profile_mut<'a>(c: &'a mut Config, id: &str) -> Result<&'a mut ProfileConfig, EditError> {
    c.profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| not_found("profile", id))
}

/// The profile holding a page, and the page's index in it.
fn page_location(c: &Config, page_id: &str) -> Result<(usize, usize), EditError> {
    c.profiles
        .iter()
        .enumerate()
        .find_map(|(p, profile)| {
            profile
                .pages
                .iter()
                .position(|page| page.id == page_id)
                .map(|i| (p, i))
        })
        .ok_or_else(|| not_found("page", page_id))
}

/// (profile index, page index, button index) of a button.
fn button_location(c: &Config, button_id: &str) -> Result<(usize, usize, usize), EditError> {
    for (p, profile) in c.profiles.iter().enumerate() {
        for (g, page) in profile.pages.iter().enumerate() {
            if let Some(b) = page.buttons.iter().position(|b| b.id == button_id) {
                return Ok((p, g, b));
            }
        }
    }
    Err(not_found("button", button_id))
}

fn button_mut<'a>(c: &'a mut Config, button_id: &str) -> Result<&'a mut ButtonConfig, EditError> {
    let (p, g, b) = button_location(c, button_id)?;
    Ok(&mut c.profiles[p].pages[g].buttons[b])
}

fn empty_page(name: String) -> PageConfig {
    PageConfig {
        id: new_id(),
        name,
        buttons: Vec::new(),
    }
}

fn apply(c: &mut Config, edit: Edit) -> Result<EditOutcome, EditError> {
    let mut created = None;
    match edit {
        Edit::AddProfile { name } => {
            let id = new_id();
            c.profiles.push(ProfileConfig {
                id: id.clone(),
                name,
                grid: GridSize { cols: 5, rows: 3 },
                pages: vec![empty_page("Page 1".to_owned())],
            });
            created = Some(id);
        }
        Edit::DuplicateProfile { profile_id, name } => {
            let mut copy = profile_mut(c, &profile_id)?.clone();
            copy.id = new_id();
            copy.name = name;
            for page in &mut copy.pages {
                page.id = new_id();
                for button in &mut page.buttons {
                    button.id = new_id();
                }
            }
            created = Some(copy.id.clone());
            c.profiles.push(copy);
        }
        Edit::RenameProfile { profile_id, name } => profile_mut(c, &profile_id)?.name = name,
        Edit::DeleteProfile { profile_id } => {
            if c.profiles.len() <= 1 {
                return Err(refused("lastProfile", "the last profile cannot be deleted"));
            }
            let index = c
                .profiles
                .iter()
                .position(|p| p.id == profile_id)
                .ok_or_else(|| not_found("profile", &profile_id))?;
            c.profiles.remove(index);
            if c.active_profile_id == profile_id {
                c.active_profile_id.clone_from(&c.profiles[0].id);
            }
        }
        Edit::SetActiveProfile { profile_id } => {
            profile_mut(c, &profile_id)?;
            c.active_profile_id = profile_id;
        }
        Edit::SetGrid {
            profile_id,
            cols,
            rows,
        } => profile_mut(c, &profile_id)?.grid = GridSize { cols, rows },
        Edit::AddPage { profile_id, name } => {
            let page = empty_page(name);
            created = Some(page.id.clone());
            profile_mut(c, &profile_id)?.pages.push(page);
        }
        Edit::RenamePage { page_id, name } => {
            let (p, g) = page_location(c, &page_id)?;
            c.profiles[p].pages[g].name = name;
        }
        Edit::MovePage { page_id, index } => {
            let (p, g) = page_location(c, &page_id)?;
            let pages = &mut c.profiles[p].pages;
            let page = pages.remove(g);
            let index = usize::try_from(index)
                .unwrap_or(usize::MAX)
                .min(pages.len());
            pages.insert(index, page);
        }
        Edit::DeletePage { page_id } => {
            let (p, g) = page_location(c, &page_id)?;
            if c.profiles[p].pages.len() <= 1 {
                return Err(refused(
                    "lastPage",
                    "the last page of a profile cannot be deleted",
                ));
            }
            c.profiles[p].pages.remove(g);
        }
        Edit::AddButton {
            page_id,
            row,
            col,
            label,
            subtitle,
            icon,
            color,
            action,
        } => {
            let (p, g) = page_location(c, &page_id)?;
            let page = &mut c.profiles[p].pages[g];
            if page.buttons.iter().any(|b| b.row == row && b.col == col) {
                return Err(ConfigError::CellTaken(page_id).into());
            }
            let id = new_id();
            page.buttons.push(ButtonConfig {
                id: id.clone(),
                row,
                col,
                label,
                subtitle,
                icon,
                color,
                action,
            });
            created = Some(id);
        }
        Edit::MoveButton {
            button_id,
            page_id,
            row,
            col,
        } => move_button(c, &button_id, &page_id, row, col)?,
        Edit::SetLabel { button_id, label } => button_mut(c, &button_id)?.label = label,
        Edit::SetSubtitle {
            button_id,
            subtitle,
        } => button_mut(c, &button_id)?.subtitle = subtitle.filter(|s| !s.is_empty()),
        Edit::SetIcon { button_id, icon } => button_mut(c, &button_id)?.icon = icon,
        Edit::SetColor { button_id, color } => button_mut(c, &button_id)?.color = color,
        Edit::SetAction { button_id, action } => button_mut(c, &button_id)?.action = action,
        Edit::DeleteButton { button_id } => {
            let (p, g, b) = button_location(c, &button_id)?;
            c.profiles[p].pages[g].buttons.remove(b);
        }
    }
    Ok(EditOutcome {
        created_id: created,
    })
}

fn move_button(
    c: &mut Config,
    button_id: &str,
    page_id: &str,
    row: u32,
    col: u32,
) -> Result<(), EditError> {
    let (p, from_page, b) = button_location(c, button_id)?;
    let (target_profile, to_page) = page_location(c, page_id)?;
    if target_profile != p {
        return Err(refused(
            "otherProfile",
            "buttons move within their profile only",
        ));
    }
    let pages = &mut c.profiles[p].pages;
    let mut button = pages[from_page].buttons.remove(b);
    // The button already in the target cell, if any, takes the vacated cell.
    if let Some(i) = pages[to_page]
        .buttons
        .iter()
        .position(|o| o.row == row && o.col == col)
    {
        let mut other = pages[to_page].buttons.remove(i);
        other.row = button.row;
        other.col = button.col;
        pages[from_page].buttons.push(other);
    }
    button.row = row;
    button.col = col;
    pages[to_page].buttons.push(button);
    Ok(())
}

#[cfg(test)]
mod tests {
    use padboard_actions::{Hotkey, Key};

    use super::super::seed;
    use super::*;

    fn editor() -> Editor {
        Editor::new(seed::config())
    }

    fn page_id(e: &Editor) -> String {
        e.config().profiles[0].pages[0].id.clone()
    }

    fn button<'a>(e: &'a Editor, id: &str) -> &'a ButtonConfig {
        e.config().profiles[0]
            .pages
            .iter()
            .flat_map(|p| &p.buttons)
            .find(|b| b.id == id)
            .unwrap()
    }

    fn add_button(e: &mut Editor, row: u32, col: u32) -> String {
        e.apply(Edit::AddButton {
            page_id: page_id(e),
            row,
            col,
            label: "New".into(),
            subtitle: None,
            icon: None,
            color: Category::Blue,
            action: Action::Hotkey(Hotkey::new([Key::F14])),
        })
        .unwrap()
        .created_id
        .unwrap()
    }

    #[test]
    fn adding_a_button_creates_it_and_can_be_undone() {
        let mut e = editor();
        let before = e.config().clone();
        let id = add_button(&mut e, 2, 3);
        assert_eq!(button(&e, &id).label, "New");
        assert!(e.undo());
        assert_eq!(*e.config(), before);
        assert!(e.redo());
        assert_eq!(button(&e, &id).label, "New");
    }

    #[test]
    fn invalid_edits_change_nothing() {
        let mut e = editor();
        let before = e.config().clone();
        let page = page_id(&e);
        let taken = e.apply(Edit::AddButton {
            page_id: page,
            row: 0,
            col: 0,
            label: "X".into(),
            subtitle: None,
            icon: None,
            color: Category::Blue,
            action: Action::Hotkey(Hotkey::new([Key::F14])),
        });
        assert_eq!(taken.unwrap_err().code, "cellTaken");
        let shrink = e.apply(Edit::SetGrid {
            profile_id: "demo".into(),
            cols: 2,
            rows: 2,
        });
        assert_eq!(shrink.unwrap_err().code, "outsideGrid");
        let long = e.apply(Edit::SetLabel {
            button_id: "copy".into(),
            label: "x".repeat(41),
        });
        assert_eq!(long.unwrap_err().code, "invalidText");
        assert_eq!(
            e.apply(Edit::DeleteButton {
                button_id: "nope".into()
            })
            .unwrap_err()
            .code,
            "notFound"
        );
        assert_eq!(*e.config(), before);
        assert!(!e.can_undo());
    }

    #[test]
    fn typing_in_one_field_is_one_undo_entry() {
        let mut e = editor();
        for label in ["C", "Co", "Cop", "Copy!"] {
            e.apply(Edit::SetLabel {
                button_id: "copy".into(),
                label: label.into(),
            })
            .unwrap();
        }
        // Another field breaks the merge.
        e.apply(Edit::SetColor {
            button_id: "copy".into(),
            color: Category::Red,
        })
        .unwrap();
        e.apply(Edit::SetLabel {
            button_id: "copy".into(),
            label: "Copy!!".into(),
        })
        .unwrap();

        assert!(e.undo());
        assert_eq!(button(&e, "copy").label, "Copy!");
        assert!(e.undo());
        assert_eq!(button(&e, "copy").color, Category::Blue);
        assert!(e.undo());
        assert_eq!(button(&e, "copy").label, "Copier");
        assert!(!e.can_undo());
    }

    #[test]
    fn a_checkpoint_ends_the_merge() {
        let mut e = editor();
        let set = |e: &mut Editor, l: &str| {
            e.apply(Edit::SetLabel {
                button_id: "copy".into(),
                label: l.into(),
            })
            .unwrap();
        };
        set(&mut e, "A");
        e.checkpoint();
        set(&mut e, "AB");
        assert!(e.undo());
        assert_eq!(button(&e, "copy").label, "A");
    }

    #[test]
    fn moving_onto_a_taken_cell_swaps() {
        let mut e = editor();
        let page = page_id(&e);
        e.apply(Edit::MoveButton {
            button_id: "copy".into(),
            page_id: page,
            row: 0,
            col: 2,
        })
        .unwrap();
        assert_eq!((button(&e, "copy").row, button(&e, "copy").col), (0, 2));
        assert_eq!((button(&e, "paste").row, button(&e, "paste").col), (0, 1));
    }

    #[test]
    fn buttons_move_and_swap_across_pages() {
        let mut e = editor();
        let second = e
            .apply(Edit::AddPage {
                profile_id: "demo".into(),
                name: "Page 2".into(),
            })
            .unwrap()
            .created_id
            .unwrap();
        e.apply(Edit::MoveButton {
            button_id: "copy".into(),
            page_id: second.clone(),
            row: 1,
            col: 1,
        })
        .unwrap();
        let pages = &e.config().profiles[0].pages;
        assert!(pages[1].buttons.iter().any(|b| b.id == "copy"));
        assert!(!pages[0].buttons.iter().any(|b| b.id == "copy"));
        // Back onto "paste"'s cell on page 1: they swap across pages.
        let first = page_id(&e);
        e.apply(Edit::MoveButton {
            button_id: "copy".into(),
            page_id: first,
            row: 0,
            col: 2,
        })
        .unwrap();
        let pages = &e.config().profiles[0].pages;
        assert!(
            pages[0]
                .buttons
                .iter()
                .any(|b| b.id == "copy" && b.col == 2)
        );
        assert!(
            pages[1]
                .buttons
                .iter()
                .any(|b| b.id == "paste" && (b.row, b.col) == (1, 1))
        );
    }

    #[test]
    fn profiles_and_pages_lifecycle() {
        let mut e = editor();
        let copy = e
            .apply(Edit::DuplicateProfile {
                profile_id: "demo".into(),
                name: "Copie".into(),
            })
            .unwrap()
            .created_id
            .unwrap();
        e.apply(Edit::SetActiveProfile {
            profile_id: copy.clone(),
        })
        .unwrap();
        assert_eq!(e.config().active_layout().layout.profile.name, "Copie");
        // Deleting the active profile falls back to the first one.
        e.apply(Edit::DeleteProfile { profile_id: copy }).unwrap();
        assert_eq!(e.config().active_profile_id, "demo");
        assert_eq!(
            e.apply(Edit::DeleteProfile {
                profile_id: "demo".into()
            })
            .unwrap_err()
            .code,
            "lastProfile"
        );
        let page = page_id(&e);
        assert_eq!(
            e.apply(Edit::DeletePage { page_id: page })
                .unwrap_err()
                .code,
            "lastPage"
        );
        let p2 = e
            .apply(Edit::AddPage {
                profile_id: "demo".into(),
                name: "B".into(),
            })
            .unwrap()
            .created_id
            .unwrap();
        e.apply(Edit::MovePage {
            page_id: p2.clone(),
            index: 0,
        })
        .unwrap();
        assert_eq!(e.config().profiles[0].pages[0].id, p2);
    }

    #[test]
    fn redo_is_dropped_by_a_new_edit_and_history_is_bounded() {
        let mut e = editor();
        add_button(&mut e, 2, 3);
        e.undo();
        assert!(e.can_redo());
        add_button(&mut e, 2, 3);
        assert!(!e.can_redo());
        for i in 0..(HISTORY + 20) {
            e.apply(Edit::SetColor {
                button_id: "copy".into(),
                color: if i % 2 == 0 {
                    Category::Red
                } else {
                    Category::Blue
                },
            })
            .unwrap();
        }
        assert_eq!(e.undo.len(), HISTORY);
    }

    /// The JSON the frontend sends (generated TypeScript types) decodes.
    #[test]
    fn frontend_edits_decode() {
        let cases = [
            r#"{"type":"setLabel","params":{"buttonId":"copy","label":"Copier"}}"#,
            r#"{"type":"setSubtitle","params":{"buttonId":"copy"}}"#,
            r#"{"type":"setIcon","params":{"buttonId":"copy","icon":{"name":"Mic","paths":["M12 19v3"]}}}"#,
            r#"{"type":"setColor","params":{"buttonId":"copy","color":"red"}}"#,
            r#"{"type":"setAction","params":{"buttonId":"copy","action":{"type":"hotkey","params":{"keys":["ControlLeft","KeyC"]}}}}"#,
            r#"{"type":"moveButton","params":{"buttonId":"copy","pageId":"page-1","row":2,"col":3}}"#,
            r#"{"type":"addButton","params":{"pageId":"page-1","row":0,"col":1,"label":"X","color":"blue","action":{"type":"hotkey","params":{"keys":[]}}}}"#,
            r#"{"type":"setGrid","params":{"profileId":"demo","cols":6,"rows":4}}"#,
            r#"{"type":"addProfile","params":{"name":"Gaming"}}"#,
            r#"{"type":"setActiveProfile","params":{"profileId":"demo"}}"#,
            r#"{"type":"movePage","params":{"pageId":"page-1","index":0}}"#,
        ];
        let mut e = editor();
        for json in cases {
            let edit: Edit =
                serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
            e.apply(edit)
                .unwrap_or_else(|err| panic!("{json}: {err:?}"));
        }
        assert_eq!(button(&e, "copy").color, Category::Red);
        assert_eq!(button(&e, "copy").subtitle, None);
    }
}
