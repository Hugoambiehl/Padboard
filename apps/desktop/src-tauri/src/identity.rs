//! Stable identity of this PC (`serverId` in `welcome`), created on first launch.
//!
//! Tablets only present their token to the PC they paired with, so the id must
//! survive restarts; a new id (deleted config) makes paired tablets pair again.

use serde::{Deserialize, Serialize};

use crate::storage::{ConfigDir, Document};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub server_id: String,
}

impl Document for Identity {
    const FILE: &'static str = "identity.json";
    const VERSION: u32 = 1;
}

impl Identity {
    /// Loads the identity, creating it on first launch.
    #[must_use]
    pub fn load_or_create(config: &ConfigDir) -> Self {
        config.load_or_init(|| Self {
            server_id: uuid::Uuid::new_v4().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_created_once_then_reused() {
        let tmp = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(tmp.path()).unwrap();
        let first = Identity::load_or_create(&config);
        assert_eq!(first.server_id.len(), 36);
        assert_eq!(Identity::load_or_create(&config), first);
    }
}
