//! Layout sent to the tablet. It only describes what to *display*: the actions
//! bound to buttons stay on the desktop, the tablet only knows button ids.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub profile: ProfileInfo,
    pub grid: GridSize,
    /// Pages of the active profile, in display order. Never empty.
    pub pages: Vec<Page>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSize {
    pub cols: u32,
    pub rows: u32,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    /// Buttons of the page. Empty cells are simply absent.
    pub buttons: Vec<ButtonView>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonView {
    /// Unique across the whole layout; sent back in `button:press` / `button:release`.
    pub id: String,
    /// Zero-based row, `< grid.rows`.
    pub row: u32,
    /// Zero-based column, `< grid.cols`.
    pub col: u32,
    /// Text displayed under the icon. Already localized by the desktop.
    pub label: String,
    /// Optional secondary line (e.g. "Scène 2", "-5 %").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub category: Category,
    /// Icon drawn above the label. Absent: the tablet shows the label's initial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ButtonIcon>,
}

/// A stroke icon on a 24 × 24 grid, Lucide style: drawn with a 2-unit stroke
/// in the category color, round caps and joins, no fill. Each entry is SVG
/// path data (`d` attribute). The desktop validates it before sending.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonIcon {
    pub paths: Vec<String>,
}

/// Color category of a button. The tablet maps it to its theme colors
/// (icon color + background at ~16 % opacity).
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Violet,
    Blue,
    Teal,
    Orange,
    Pink,
    Red,
    Grey,
}

impl Layout {
    /// Checks the invariants the tablet relies on: at least one page, buttons
    /// inside the grid, unique button ids and at most one button per cell.
    ///
    /// # Errors
    /// Returns a developer-oriented description of the first violation found.
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::HashSet;

        if self.grid.cols == 0 || self.grid.rows == 0 {
            return Err(format!(
                "grid must be at least 1x1, got {}x{}",
                self.grid.cols, self.grid.rows
            ));
        }
        if self.pages.is_empty() {
            return Err("layout must contain at least one page".into());
        }
        let mut ids = HashSet::new();
        for page in &self.pages {
            let mut cells = HashSet::new();
            for b in &page.buttons {
                if b.row >= self.grid.rows || b.col >= self.grid.cols {
                    return Err(format!(
                        "button {} at ({}, {}) is outside the {}x{} grid",
                        b.id, b.row, b.col, self.grid.cols, self.grid.rows
                    ));
                }
                if !ids.insert(b.id.as_str()) {
                    return Err(format!("duplicate button id {}", b.id));
                }
                if !cells.insert((b.row, b.col)) {
                    return Err(format!(
                        "page {} has two buttons at ({}, {})",
                        page.id, b.row, b.col
                    ));
                }
            }
        }
        Ok(())
    }
}
