//! The editable configuration: profiles → pages and folders → buttons. Persisted in
//! `profiles.json` (versioned, `storage.rs`), edited only through the
//! operations of `ops.rs`, and mirrored in TypeScript by typeshare
//! (`apps/desktop/src/generated/types.ts`).
//!
//! Type names end with `Config` so they never clash with the protocol types
//! in the generated TypeScript (`Page`, `ButtonView`…).

use padboard_actions::Action;
use padboard_protocol::{Category, GridSize};
use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The profile shown on the tablets.
    pub active_profile_id: String,
    pub profiles: Vec<ProfileConfig>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfig {
    pub id: String,
    pub name: String,
    pub grid: GridSize,
    /// At least one. Swiped through on the tablet.
    pub pages: Vec<PageConfig>,
    /// Pages outside the swipe order, opened by a navigation button.
    pub folders: Vec<PageConfig>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageConfig {
    pub id: String,
    pub name: String,
    pub buttons: Vec<ButtonConfig>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonConfig {
    /// Unique across the whole configuration (sent in `button:press`).
    pub id: String,
    pub row: u32,
    pub col: u32,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconConfig>,
    pub color: Category,
    pub action: Action,
}

/// A Lucide icon: its name (for the icon picker) and its SVG path data (what
/// the tablet draws, see the protocol's `ButtonIcon`).
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconConfig {
    /// Key in the `lucide` package's `icons` export, e.g. `"Mic"`, `"Volume2"`.
    pub name: String,
    pub paths: Vec<String>,
}
