//! Updates: the desktop app's own (`tauri-plugin-updater` on Windows, a
//! notice with the release page on Linux, where the `.deb` is updated by
//! hand) and, later, the tablets' (the APK of the same release, pushed by
//! this PC). Everything downloaded is checked against signatures made on the
//! maintainer's PC ([`signing`]).
//!
//! The check runs a few seconds after launch, then once a day, unless the
//! user turned it off ("Mises à jour" card). It only asks GitHub for the
//! list of releases: nothing about this PC is sent.

pub mod assets;
pub mod release;
pub mod signing;
pub mod source;
pub mod tablet;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use semver::Version;
use serde::{Deserialize, Serialize};

use self::release::ApiRelease;
use self::source::Source;

/// First check after launch (the app settles first), then daily.
const FIRST_CHECK: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Releases looked at (the newest first).
const RELEASES_LISTED: u32 = 30;

/// The user's choices, persisted in `preferences.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdatePreferences {
    /// Look for a new version at launch and daily. On by default.
    pub check: bool,
    /// Release candidates too ("Versions de test").
    pub prereleases: bool,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            check: true,
            prereleases: false,
        }
    }
}

/// Why updates cannot be looked for at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OffReason {
    /// This build has no update key (`updater-key.pub`).
    NoKey,
    /// Test mode never contacts GitHub.
    TestMode,
}

/// What went wrong (translated by the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateError {
    /// GitHub could not be reached or answered something unexpected.
    Network,
    /// The download is not signed by the release key for this version.
    Signature,
    /// The installer could not be started.
    Install,
}

/// Where the updater is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateStatus {
    Off {
        reason: OffReason,
    },
    /// Not checked yet.
    Idle,
    Checking,
    /// `checked_at`: milliseconds since the Unix epoch.
    UpToDate {
        checked_at: u64,
    },
    /// `installable`: this PC installs it in one click (Windows, release
    /// signed for the updater); otherwise the release page.
    Available {
        version: String,
        installable: bool,
    },
    Downloading {
        version: String,
        received: u64,
        total: Option<u64>,
    },
    /// The installer runs; the app is about to exit.
    Installing {
        version: String,
    },
    Failed {
        error: UpdateError,
    },
}

/// Everything the "Mises à jour" card shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesView {
    pub current: String,
    pub preferences: UpdatePreferences,
    pub status: UpdateStatus,
}

/// Errors of the commands (the UI shows them as they are: unexpected).
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("updates are off in this build or mode")]
    Off,
    #[error("busy")]
    Busy,
    #[error("no installable update")]
    NotInstallable,
}

struct Inner {
    preferences: UpdatePreferences,
    status: UpdateStatus,
}

type Notify = Box<dyn Fn(UpdatesView) + Send + Sync>;

/// Looks for updates and installs them. Cheap to clone.
#[derive(Clone)]
pub struct Updates(Arc<Shared>);

impl std::fmt::Debug for Updates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Updates").field(&self.view()).finish()
    }
}

struct Shared {
    /// Where releases come from (GitHub, or a local server in test builds).
    source: Source,
    current: Version,
    inner: Mutex<Inner>,
    notify: Notify,
    /// `None` when off (no key, test mode).
    http: Option<reqwest::Client>,
    /// Wakes the background loop (preferences changed, "Rechercher").
    wake: tokio::sync::Notify,
}

impl Updates {
    /// Updates from [`Source::current`]. Off in test mode (except with a test
    /// source) and without a key. `notify` gets every change.
    #[must_use]
    pub fn new(preferences: UpdatePreferences, test_mode: bool, notify: Notify) -> Self {
        let source = Source::current();
        let off = if test_mode && !source.test {
            Some(OffReason::TestMode)
        } else if source.key().is_none() {
            Some(OffReason::NoKey)
        } else {
            None
        };
        Self::with_source(preferences, off, source, notify)
    }

    fn with_source(
        preferences: UpdatePreferences,
        off: Option<OffReason>,
        source: Source,
        notify: Notify,
    ) -> Self {
        let http = if off.is_none() { client() } else { None };
        let status = match (off, &http) {
            (Some(reason), _) => UpdateStatus::Off { reason },
            (None, None) => UpdateStatus::Failed {
                error: UpdateError::Network,
            },
            (None, Some(_)) => UpdateStatus::Idle,
        };
        if let Some(reason) = off {
            tracing::info!(?reason, "updates are off");
        }
        Self(Arc::new(Shared {
            current: Version::parse(&source.version).unwrap_or_else(|_| Version::new(0, 0, 0)),
            source,
            inner: Mutex::new(Inner {
                preferences,
                status,
            }),
            notify,
            http,
            wake: tokio::sync::Notify::new(),
        }))
    }

    #[must_use]
    pub fn view(&self) -> UpdatesView {
        let inner = self.0.inner.lock().unwrap_or_else(PoisonError::into_inner);
        UpdatesView {
            current: self.0.current.to_string(),
            preferences: inner.preferences,
            status: inner.status.clone(),
        }
    }

    fn set_status(&self, status: UpdateStatus) {
        {
            let mut inner = self.0.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if inner.status == status {
                return;
            }
            inner.status = status;
        }
        (self.0.notify)(self.view());
    }

    fn status(&self) -> UpdateStatus {
        self.0
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .status
            .clone()
    }

    /// New preferences (already saved by the caller). Turning the check on,
    /// or changing the channel while it is on, checks at once.
    pub fn set_preferences(&self, preferences: UpdatePreferences) {
        let check_now = {
            let mut inner = self.0.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let before = inner.preferences;
            inner.preferences = preferences;
            preferences.check && before != preferences
        };
        (self.0.notify)(self.view());
        if check_now {
            self.0.wake.notify_one();
        }
    }

    /// Checks now ("Rechercher"), whatever the automatic check setting.
    ///
    /// # Errors
    /// Updates are off, or an install is under way.
    pub fn check_now(&self) -> Result<(), CommandError> {
        if self.0.http.is_none() {
            return Err(CommandError::Off);
        }
        if self.busy() {
            return Err(CommandError::Busy);
        }
        let this = self.clone();
        tauri::async_runtime::spawn(async move { this.check().await });
        Ok(())
    }

    fn busy(&self) -> bool {
        matches!(
            self.status(),
            UpdateStatus::Checking
                | UpdateStatus::Downloading { .. }
                | UpdateStatus::Installing { .. }
        )
    }

    /// Checks once at launch then daily while the check is on; also when
    /// woken by [`Updates::set_preferences`].
    pub async fn run(self) {
        if self.0.http.is_none() {
            return;
        }
        let mut wait = FIRST_CHECK;
        loop {
            let woken = tokio::select! {
                () = tokio::time::sleep(wait) => false,
                () = self.0.wake.notified() => true,
            };
            let enabled = self
                .0
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .preferences
                .check;
            if enabled && !self.busy() {
                self.check().await;
            }
            if !woken {
                wait = CHECK_EVERY;
            }
        }
    }

    /// Asks GitHub for the releases and reports the newest above this one.
    async fn check(&self) {
        let Some(http) = &self.0.http else { return };
        self.set_status(UpdateStatus::Checking);
        let prereleases = self
            .0
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .preferences
            .prereleases;
        let status = match fetch_releases(http, &self.0.source.api_url).await {
            Ok(releases) => match release::newest(&releases, &self.0.current, prereleases) {
                Some(newer) => {
                    tracing::info!(version = %newer.version, "update available");
                    UpdateStatus::Available {
                        version: newer.version.to_string(),
                        installable: cfg!(windows) && newer.has_manifest,
                    }
                }
                None => {
                    tracing::info!(prereleases, "no update");
                    UpdateStatus::UpToDate {
                        checked_at: now_millis(),
                    }
                }
            },
            Err(e) => {
                tracing::warn!("update check failed: {e}");
                UpdateStatus::Failed {
                    error: UpdateError::Network,
                }
            }
        };
        self.set_status(status);
    }

    /// Downloads and installs the available update (Windows): the app exits
    /// once the installer has started; `before_exit` saves what must be.
    ///
    /// # Errors
    /// Nothing installable, or busy. Failures of the install itself are
    /// reported in the status.
    #[cfg_attr(not(windows), allow(clippy::needless_pass_by_value))]
    pub fn install<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        before_exit: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), CommandError> {
        let version = self.installable_version()?;
        #[cfg(windows)]
        {
            let this = self.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = windows::install(&this, &app, &version, before_exit).await {
                    this.set_status(UpdateStatus::Failed { error });
                }
            });
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = (app, version, before_exit);
            Err(CommandError::NotInstallable)
        }
    }
}

impl Updates {
    /// The page "Voir la version" opens: the available release, else the
    /// list of releases. Built here, never taken from the UI.
    #[must_use]
    pub fn release_page(&self) -> String {
        match self.status() {
            UpdateStatus::Available { version, .. }
            | UpdateStatus::Downloading { version, .. }
            | UpdateStatus::Installing { version } => self.0.source.page(Some(&version)),
            _ => self.0.source.page(None),
        }
    }

    /// The version "Installer" would install now.
    fn installable_version(&self) -> Result<String, CommandError> {
        match self.status() {
            UpdateStatus::Available {
                version,
                installable: true,
            } => Ok(version),
            _ if self.busy() => Err(CommandError::Busy),
            _ => Err(CommandError::NotInstallable),
        }
    }
}

#[cfg(windows)]
mod windows {
    use tauri_plugin_updater::UpdaterExt as _;

    use super::{UpdateError, UpdateStatus, Updates, assets};

    pub(super) async fn install<R: tauri::Runtime>(
        updates: &Updates,
        app: &tauri::AppHandle<R>,
        version: &str,
        before_exit: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), UpdateError> {
        let manifest =
            assets::download_url(&updates.0.source.releases_url, version, assets::MANIFEST);
        let network = |e: tauri_plugin_updater::Error| {
            tracing::warn!("update: {e}");
            UpdateError::Network
        };
        updates.set_status(UpdateStatus::Downloading {
            version: version.to_owned(),
            received: 0,
            total: None,
        });
        let updater = app
            .updater_builder()
            .endpoints(vec![manifest.parse().map_err(|_| UpdateError::Network)?])
            .map_err(network)?
            .on_before_exit(before_exit)
            .build()
            .map_err(network)?;
        // The plugin checks again that this version is newer.
        let Some(update) = updater.check().await.map_err(network)? else {
            return Err(UpdateError::Network);
        };
        let mut received = 0u64;
        let progress = updates.clone();
        let announced = update.version.clone();
        let bytes = update
            .download(
                |chunk, total| {
                    received += chunk as u64;
                    progress.set_status(UpdateStatus::Downloading {
                        version: announced.clone(),
                        received,
                        total,
                    });
                },
                || {},
            )
            .await
            .map_err(|e| {
                tracing::error!("update refused: {e}");
                match e {
                    tauri_plugin_updater::Error::Minisign(_)
                    | tauri_plugin_updater::Error::SignatureUtf8(_)
                    | tauri_plugin_updater::Error::Base64(_)
                    | tauri_plugin_updater::Error::MissingSignedVersion
                    | tauri_plugin_updater::Error::SignedVersionMismatch { .. } => {
                        UpdateError::Signature
                    }
                    _ => UpdateError::Network,
                }
            })?;
        updates.set_status(UpdateStatus::Installing {
            version: update.version.clone(),
        });
        tracing::info!(version = %update.version, "installing the update, the app exits");
        update.install(bytes).map_err(|e| {
            tracing::error!("cannot start the installer: {e}");
            UpdateError::Install
        })
    }
}

/// The HTTP client of update checks and downloads (`None` if TLS cannot be
/// set up, which leaves updates failed).
pub(crate) fn client() -> Option<reqwest::Client> {
    // reqwest is built without a crypto provider: use ring, like the rest of
    // the app (no-op if already installed).
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .user_agent(concat!("Padboard/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        // Whole requests: an APK takes longer than a list of releases.
        .timeout(Duration::from_secs(300))
        .build()
        .inspect_err(|e| tracing::error!("cannot create the HTTP client: {e}"))
        .ok()
}

async fn fetch_releases(http: &reqwest::Client, api_url: &str) -> Result<Vec<ApiRelease>, String> {
    let response = http
        .get(format!("{api_url}?per_page={RELEASES_LISTED}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    /// Answers every HTTP request with `body` (status 200), counting them.
    async fn serve(body: &'static str) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/releases", listener.local_addr().unwrap());
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        (url, hits)
    }

    fn source(api_url: String) -> Source {
        Source {
            api_url,
            releases_url: String::new(),
            key_text: String::new(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            test: true,
        }
    }

    fn service(api_url: String) -> (Updates, mpsc::Receiver<UpdatesView>) {
        let (tx, rx) = mpsc::channel();
        let updates = Updates::with_source(
            UpdatePreferences::default(),
            None,
            source(api_url),
            Box::new(move |view| {
                let _ = tx.send(view);
            }),
        );
        (updates, rx)
    }

    #[tokio::test]
    async fn a_newer_release_is_reported() {
        let (url, _) = serve(
            r#"[{"tag_name":"v99.0.0","draft":false,"prerelease":false,"assets":[{"name":"latest.json"}]},
                {"tag_name":"v99.1.0-rc.1","draft":false,"prerelease":true,"assets":[]}]"#,
        )
        .await;
        let (updates, _rx) = service(url);
        updates.check().await;
        assert_eq!(
            updates.view().status,
            UpdateStatus::Available {
                version: "99.0.0".into(),
                installable: cfg!(windows),
            }
        );

        updates.set_preferences(UpdatePreferences {
            check: true,
            prereleases: true,
        });
        updates.check().await;
        assert_eq!(
            updates.view().status,
            UpdateStatus::Available {
                version: "99.1.0-rc.1".into(),
                installable: false,
            }
        );
    }

    #[tokio::test]
    async fn up_to_date_and_failures() {
        let (url, _) = serve(r#"[{"tag_name":"v0.0.1","assets":[]}]"#).await;
        let (updates, _rx) = service(url);
        updates.check().await;
        assert!(matches!(
            updates.view().status,
            UpdateStatus::UpToDate { .. }
        ));

        let (url, _) = serve("not json").await;
        let (updates, _rx) = service(url);
        updates.check().await;
        assert_eq!(
            updates.view().status,
            UpdateStatus::Failed {
                error: UpdateError::Network
            }
        );
    }

    #[tokio::test]
    async fn off_never_contacts_github() {
        let (url, hits) = serve("[]").await;
        let updates = Updates::with_source(
            UpdatePreferences::default(),
            Some(OffReason::TestMode),
            source(url),
            Box::new(|_| {}),
        );
        assert!(matches!(updates.check_now(), Err(CommandError::Off)));
        updates.check().await;
        tokio::time::timeout(Duration::from_millis(50), updates.clone().run())
            .await
            .expect("run returns at once when off");
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            updates.view().status,
            UpdateStatus::Off {
                reason: OffReason::TestMode
            }
        );
    }

    #[tokio::test]
    async fn turning_the_check_on_wakes_the_loop() {
        let (url, hits) = serve("[]").await;
        let (updates, rx) = service(url);
        updates.set_preferences(UpdatePreferences {
            check: false,
            prereleases: false,
        });
        tokio::spawn(updates.clone().run());
        updates.set_preferences(UpdatePreferences::default());
        // Woken long before FIRST_CHECK.
        tokio::time::timeout(Duration::from_secs(5), async {
            while !matches!(updates.view().status, UpdateStatus::UpToDate { .. }) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(rx.try_iter().count() >= 3);
    }

    #[test]
    fn only_an_installable_update_is_installed() {
        let updates = Updates::with_source(
            UpdatePreferences::default(),
            Some(OffReason::NoKey),
            source(String::new()),
            Box::new(|_| {}),
        );
        assert!(matches!(
            updates.installable_version(),
            Err(CommandError::NotInstallable)
        ));
        updates.set_status(UpdateStatus::Available {
            version: "9.0.0".into(),
            installable: false,
        });
        assert!(matches!(
            updates.installable_version(),
            Err(CommandError::NotInstallable)
        ));
        updates.set_status(UpdateStatus::Downloading {
            version: "9.0.0".into(),
            received: 0,
            total: None,
        });
        assert!(matches!(
            updates.installable_version(),
            Err(CommandError::Busy)
        ));
        updates.set_status(UpdateStatus::Available {
            version: "9.0.0".into(),
            installable: true,
        });
        assert_eq!(updates.installable_version().unwrap(), "9.0.0");
    }

    #[test]
    fn preferences_default_to_checking_stable_releases() {
        assert_eq!(
            UpdatePreferences::default(),
            UpdatePreferences {
                check: true,
                prereleases: false
            }
        );
    }
}
