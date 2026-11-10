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

        let changed = Preferences {
            lan_enabled: true,
            adb_path: Some("/opt/adb".into()),
        };
        config.save(&changed).unwrap();
        assert_eq!(config.load::<Preferences>().unwrap(), Some(changed));
    }
}
