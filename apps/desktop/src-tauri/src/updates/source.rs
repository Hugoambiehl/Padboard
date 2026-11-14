//! Where releases come from and what checks them: GitHub, the update key
//! embedded in the app and this build's version.
//!
//! **Test builds only**: built with the `test-updates` Cargo feature, the
//! app reads `PADBOARD_TEST_RELEASES` (a local server: `<url>/api/releases`
//! for the list, `<url>/releases/download/v<X>/<file>` for the files),
//! `PADBOARD_TEST_UPDATE_KEY` (a test public key file) and
//! `PADBOARD_TEST_APP_VERSION` (the version it pretends to be), so a real
//! update can be tested without publishing anything. Shipped builds never
//! have the feature: the code reading those variables is not compiled in
//! (test `shipped_builds_never_take_a_test_source`, and the release
//! workflow checks the binaries do not even contain the variable names).

use super::assets;
use super::signing::UpdateKey;

/// Releases, key and version used by the updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The releases API (the list).
    pub api_url: String,
    /// `…/releases`: pages and `download/v<X>/<file>`.
    pub releases_url: String,
    /// The public key, as the Tauri CLI writes it (empty: none).
    pub key_text: String,
    /// The version this app is (and offers to tablets).
    pub version: String,
    /// A test source (`test-updates` builds): updates work in test mode.
    pub test: bool,
}

impl Source {
    /// The source of this build: GitHub, unless a test build is told otherwise.
    #[must_use]
    pub fn current() -> Self {
        #[cfg(feature = "test-updates")]
        if let Some(source) = Self::from_test_environment() {
            tracing::warn!(url = %source.releases_url, version = %source.version, "TEST UPDATES: releases from a local server, checked with a test key");
            return source;
        }
        Self::github()
    }

    /// GitHub, the embedded key, this build's version.
    #[must_use]
    pub fn github() -> Self {
        Self {
            api_url: assets::RELEASES_API.to_owned(),
            releases_url: assets::RELEASES_URL.to_owned(),
            key_text: UpdateKey::embedded_text().to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            test: false,
        }
    }

    /// The key that checks this source's signatures (`None`: updates off).
    #[must_use]
    pub fn key(&self) -> Option<UpdateKey> {
        if self.key_text.trim().is_empty() {
            return None;
        }
        UpdateKey::from_tauri(&self.key_text)
            .inspect_err(|e| tracing::error!("the update key is unusable: {e}"))
            .ok()
    }

    /// The page of `version` (or of every release).
    #[must_use]
    pub fn page(&self, version: Option<&str>) -> String {
        match version {
            Some(version) => format!("{}/tag/{}", self.releases_url, assets::tag(version)),
            None => self.releases_url.clone(),
        }
    }

    #[cfg(feature = "test-updates")]
    fn from_test_environment() -> Option<Self> {
        let base = std::env::var("PADBOARD_TEST_RELEASES").ok()?;
        let base = base.trim_end_matches('/');
        let key_text = std::env::var("PADBOARD_TEST_UPDATE_KEY")
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default();
        let version = std::env::var("PADBOARD_TEST_APP_VERSION")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned());
        Some(Self {
            api_url: format!("{base}/api/releases"),
            releases_url: format!("{base}/releases"),
            key_text: key_text.trim().to_owned(),
            version,
            test: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shipped build (no `test-updates` feature) ignores the test
    /// variables: GitHub, the embedded key and its own version, always.
    #[cfg(not(feature = "test-updates"))]
    #[test]
    fn shipped_builds_never_take_a_test_source() {
        // SAFETY: only this module reads these variables, and only in
        // builds with the feature, which this test is not compiled in.
        unsafe {
            std::env::set_var("PADBOARD_TEST_RELEASES", "http://127.0.0.1:9/evil");
            std::env::set_var("PADBOARD_TEST_UPDATE_KEY", "/nonexistent");
            std::env::set_var("PADBOARD_TEST_APP_VERSION", "99.0.0");
        }
        let source = Source::current();
        assert_eq!(source, Source::github());
        assert!(!source.test);
        assert!(
            source
                .api_url
                .starts_with("https://api.github.com/repos/Hugoambiehl/Padboard/")
        );
        assert!(
            source
                .releases_url
                .starts_with("https://github.com/Hugoambiehl/Padboard/")
        );
        assert_eq!(source.version, env!("CARGO_PKG_VERSION"));
    }

    #[cfg(feature = "test-updates")]
    #[test]
    fn test_builds_take_the_local_server() {
        let tmp = tempfile::tempdir().unwrap();
        let key = tmp.path().join("test.pub");
        std::fs::write(&key, include_str!("fixtures/test-key.pub")).unwrap();
        // SAFETY: only this module reads these variables.
        unsafe {
            std::env::set_var("PADBOARD_TEST_RELEASES", "http://127.0.0.1:8765/");
            std::env::set_var("PADBOARD_TEST_UPDATE_KEY", &key);
            std::env::set_var("PADBOARD_TEST_APP_VERSION", "0.2.0-rc.1");
        }
        let source = Source::current();
        assert!(source.test);
        assert_eq!(source.api_url, "http://127.0.0.1:8765/api/releases");
        assert_eq!(source.version, "0.2.0-rc.1");
        assert!(source.key().is_some());
    }

    #[test]
    fn pages() {
        let github = Source::github();
        assert_eq!(
            github.page(Some("0.2.0")),
            "https://github.com/Hugoambiehl/Padboard/releases/tag/v0.2.0"
        );
        assert_eq!(github.page(None), assets::RELEASES_URL);
    }
}
