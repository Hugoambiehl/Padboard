//! Invariants of a [`Config`], checked after every edit and on load.

use std::collections::HashSet;

use serde::Serialize;
use typeshare::typeshare;

use super::model::{ButtonConfig, Config, IconConfig, ProfileConfig};
use super::svg_path;

pub const MAX_PROFILES: usize = 20;
pub const MAX_PAGES: usize = 20;
pub const MAX_FOLDERS: usize = 20;
pub const MIN_GRID: u32 = 1;
pub const MAX_COLS: u32 = 8;
pub const MAX_ROWS: u32 = 6;
/// Names of profiles and pages, labels and subtitles (characters).
pub const MAX_TEXT: usize = 40;
pub const MAX_ID: usize = 64;
/// Icons: Lucide's largest needs 20 paths and 712 bytes; generous margins.
pub const MAX_ICON_PATHS: usize = 32;
pub const MAX_PATH_LEN: usize = 1024;
pub const MAX_ICON_BYTES: usize = 2048;

/// Why a configuration is refused. `code()` is stable, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("no profile")]
    NoProfile,
    #[error("too many profiles (max {MAX_PROFILES})")]
    TooManyProfiles,
    #[error("the active profile {0:?} does not exist")]
    UnknownActiveProfile(String),
    #[error("profile {0:?} has no page")]
    NoPage(String),
    #[error("profile {0:?} has too many pages (max {MAX_PAGES})")]
    TooManyPages(String),
    #[error("profile {0:?} has too many folders (max {MAX_FOLDERS})")]
    TooManyFolders(String),
    #[error("grid {cols}×{rows} out of bounds (1–{MAX_COLS} × 1–{MAX_ROWS})")]
    GridSize { cols: u32, rows: u32 },
    #[error("button {0:?} is outside the grid")]
    OutsideGrid(String),
    #[error("two buttons share a cell on page {0:?}")]
    CellTaken(String),
    #[error("duplicate id {0:?}")]
    DuplicateId(String),
    #[error("invalid id {0:?}")]
    InvalidId(String),
    #[error("text too long or empty: {0:?}")]
    InvalidText(String),
    #[error("invalid icon on button {0:?}")]
    InvalidIcon(String),
    #[error("invalid action on button {button:?}: {detail}")]
    InvalidAction { button: String, detail: String },
}

impl ConfigError {
    /// Stable identifier the UI translates.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoProfile => "noProfile",
            Self::TooManyProfiles => "tooManyProfiles",
            Self::UnknownActiveProfile(_) => "unknownActiveProfile",
            Self::NoPage(_) => "noPage",
            Self::TooManyPages(_) => "tooManyPages",
            Self::TooManyFolders(_) => "tooManyFolders",
            Self::GridSize { .. } => "gridSize",
            Self::OutsideGrid(_) => "outsideGrid",
            Self::CellTaken(_) => "cellTaken",
            Self::DuplicateId(_) => "duplicateId",
            Self::InvalidId(_) => "invalidId",
            Self::InvalidText(_) => "invalidText",
            Self::InvalidIcon(_) => "invalidIcon",
            Self::InvalidAction { .. } => "invalidAction",
        }
    }
}

/// What the frontend receives when an edit is refused: `code` is stable (the
/// UI translates it), `detail` is the English developer message.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditError {
    pub code: String,
    pub detail: String,
}

impl From<ConfigError> for EditError {
    fn from(e: ConfigError) -> Self {
        Self {
            code: e.code().to_owned(),
            detail: e.to_string(),
        }
    }
}

impl Config {
    /// # Errors
    /// The first broken invariant.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.profiles.is_empty() {
            return Err(ConfigError::NoProfile);
        }
        if self.profiles.len() > MAX_PROFILES {
            return Err(ConfigError::TooManyProfiles);
        }
        if !self.profiles.iter().any(|p| p.id == self.active_profile_id) {
            return Err(ConfigError::UnknownActiveProfile(
                self.active_profile_id.clone(),
            ));
        }
        let mut ids = HashSet::new();
        for profile in &self.profiles {
            check_id(&profile.id, &mut ids)?;
            check_text(&profile.name, false)?;
            validate_profile(profile, &mut ids)?;
        }
        Ok(())
    }
}

fn validate_profile(profile: &ProfileConfig, ids: &mut HashSet<String>) -> Result<(), ConfigError> {
    let grid = profile.grid;
    if !(MIN_GRID..=MAX_COLS).contains(&grid.cols) || !(MIN_GRID..=MAX_ROWS).contains(&grid.rows) {
        return Err(ConfigError::GridSize {
            cols: grid.cols,
            rows: grid.rows,
        });
    }
    if profile.pages.is_empty() {
        return Err(ConfigError::NoPage(profile.id.clone()));
    }
    if profile.pages.len() > MAX_PAGES {
        return Err(ConfigError::TooManyPages(profile.id.clone()));
    }
    if profile.folders.len() > MAX_FOLDERS {
        return Err(ConfigError::TooManyFolders(profile.id.clone()));
    }
    // Pages and folders share the id space and the cell rules.
    for page in profile.pages.iter().chain(&profile.folders) {
        check_id(&page.id, ids)?;
        check_text(&page.name, false)?;
        let mut cells = HashSet::new();
        for button in &page.buttons {
            check_id(&button.id, ids)?;
            if button.row >= grid.rows || button.col >= grid.cols {
                return Err(ConfigError::OutsideGrid(button.id.clone()));
            }
            if !cells.insert((button.row, button.col)) {
                return Err(ConfigError::CellTaken(page.id.clone()));
            }
            validate_button(button)?;
        }
    }
    Ok(())
}

fn validate_button(button: &ButtonConfig) -> Result<(), ConfigError> {
    check_text(&button.label, true)?;
    if let Some(subtitle) = &button.subtitle {
        check_text(subtitle, true)?;
    }
    if let Some(icon) = &button.icon
        && !icon_is_valid(icon)
    {
        return Err(ConfigError::InvalidIcon(button.id.clone()));
    }
    if let Some(look) = &button.active_look {
        if let Some(label) = &look.label {
            check_text(label, true)?;
        }
        if let Some(icon) = &look.icon
            && !icon_is_valid(icon)
        {
            return Err(ConfigError::InvalidIcon(button.id.clone()));
        }
    }
    // Actions being set up (no key, no target yet) are allowed in the
    // configuration and refused when pressed (`Action::validate`). Page and
    // folder ids may dangle (deleted page): the tablet ignores them and the
    // editor shows it.
    button
        .action
        .validate_draft()
        .map_err(|e| ConfigError::InvalidAction {
            button: button.id.clone(),
            detail: e.to_string(),
        })
}

fn icon_is_valid(icon: &IconConfig) -> bool {
    let name_ok = !icon.name.is_empty()
        && icon.name.len() <= MAX_ID
        && icon.name.bytes().all(|b| b.is_ascii_alphanumeric());
    let total: usize = icon.paths.iter().map(String::len).sum();
    name_ok
        && !icon.paths.is_empty()
        && icon.paths.len() <= MAX_ICON_PATHS
        && total <= MAX_ICON_BYTES
        && icon
            .paths
            .iter()
            .all(|d| d.len() <= MAX_PATH_LEN && svg_path::is_valid(d))
}

fn check_id(id: &str, seen: &mut HashSet<String>) -> Result<(), ConfigError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok {
        return Err(ConfigError::InvalidId(id.to_owned()));
    }
    if !seen.insert(id.to_owned()) {
        return Err(ConfigError::DuplicateId(id.to_owned()));
    }
    Ok(())
}

/// `may_be_empty`: a button label may be empty (icon only), names may not.
fn check_text(text: &str, may_be_empty: bool) -> Result<(), ConfigError> {
    let len = text.chars().count();
    let ok = len <= MAX_TEXT
        && (may_be_empty || !text.trim().is_empty())
        && !text.chars().any(char::is_control);
    if ok {
        Ok(())
    } else {
        Err(ConfigError::InvalidText(
            text.chars().take(MAX_TEXT).collect(),
        ))
    }
}
