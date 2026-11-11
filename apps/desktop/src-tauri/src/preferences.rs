//! User preferences changed from the UI, persisted in `preferences.json`.
//! Environment variables (`settings.rs`) take precedence over them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::storage::Document;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    /// Accept Wi-Fi connections (TLS port, mDNS announcement). Off by default.
    #[serde(default)]
    pub lan_enabled: bool,
    /// `adb` executable chosen by the user; `None` = look it up in `PATH`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adb_path: Option<PathBuf>,
    /// The tablets' screensaver (sent to them with `tablet:settings`).
    #[serde(default)]
    pub screensaver: Screensaver,
}

/// Shortest and longest screensaver delays accepted.
pub const SCREENSAVER_DELAYS: std::ops::RangeInclusive<u32> = 15..=3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screensaver {
    /// Black screen with a clock after `delay_seconds` without a touch.
    pub enabled: bool,
    pub delay_seconds: u32,
}

impl Default for Screensaver {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_seconds: 300,
        }
    }
}

impl Screensaver {
    /// What the tablets receive.
    #[must_use]
    pub fn payload(self) -> padboard_protocol::TabletSettingsPayload {
        padboard_protocol::TabletSettingsPayload {
            screensaver: self.enabled,
            screensaver_delay_seconds: self.delay_seconds,
        }
    }
}

impl Document for Preferences {
    const FILE: &'static str = "preferences.json";
    const VERSION: u32 = 1;
}

#[cfg(test)]
mod tests {
    use crate::storage::ConfigDir;

    use super::*;

    #[test]
    fn defaults_keep_wifi_off_and_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(tmp.path()).unwrap();
        let prefs = config.load_or_init(Preferences::default);
        assert!(!prefs.lan_enabled);
        assert_eq!(prefs.adb_path, None);
        assert_eq!(
            prefs.screensaver,
            Screensaver {
                enabled: true,
                delay_seconds: 300
            }
        );

        let changed = Preferences {
            lan_enabled: true,
            adb_path: Some("/opt/adb".into()),
            screensaver: Screensaver {
                enabled: false,
                delay_seconds: 60,
            },
        };
        config.save(&changed).unwrap();
        assert_eq!(config.load::<Preferences>().unwrap(), Some(changed));
    }

    #[test]
    fn preferences_saved_before_the_screensaver_get_its_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("preferences.json"),
            r#"{ "version": 1, "lanEnabled": true }"#,
        )
        .unwrap();
        let config = ConfigDir::open(tmp.path()).unwrap();
        let prefs = config.load::<Preferences>().unwrap().unwrap();
        assert!(prefs.lan_enabled);
        assert_eq!(prefs.screensaver, Screensaver::default());
    }
}
