//! The tablet's APK of this PC's own version, from the same GitHub release:
//! downloaded once, kept in `<config>/updates/`, and checked every time it
//! is used against `SHA256SUMS`, itself signed by the update key **for this
//! version** (`signing.rs`). The tablet (and Android) then check the APK's
//! own signature before installing it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use super::assets;
use super::signing::{CHECKSUMS, CHECKSUMS_SIGNATURE, Checksums, SigningError, UpdateKey};

/// Why the APK is not available. Codes, translated by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ApkError {
    /// This build has no update key: nothing can be checked.
    #[error("no update key in this build")]
    NoKey,
    /// This PC's version has no published, signed release (a development
    /// build, or a release not signed yet).
    #[error("version {version} is not published")]
    NotPublished { version: String },
    #[error("cannot download: {detail}")]
    Network { detail: String },
    /// A file is not what the release key signed for this version.
    #[error("not signed for this version: {detail}")]
    Signature { detail: String },
    #[error("cannot store the APK: {detail}")]
    Storage { detail: String },
}

/// A checked APK, on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apk {
    pub version: String,
    pub version_code: u32,
    pub path: PathBuf,
}

/// Downloads and checks the APK of one version; one download at a time.
#[derive(Debug)]
pub struct Apks {
    key: Option<UpdateKey>,
    /// Releases (`…/releases`), where `download/v<X>/<file>` lives.
    base_url: String,
    dir: PathBuf,
    version: String,
    http: Option<reqwest::Client>,
    lock: tokio::sync::Mutex<()>,
}

impl Apks {
    /// This PC's version, from the GitHub release, kept in `dir`.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self::with(
            UpdateKey::embedded(),
            assets::RELEASES_URL.to_owned(),
            dir,
            env!("CARGO_PKG_VERSION").to_owned(),
        )
    }

    fn with(key: Option<UpdateKey>, base_url: String, dir: PathBuf, version: String) -> Self {
        Self {
            http: key.as_ref().and_then(|_| super::client()),
            key,
            base_url,
            dir,
            version,
            lock: tokio::sync::Mutex::new(()),
        }
    }

    /// The version a tablet is updated to: this PC's.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Its `versionCode` (`None` outside the release scheme, e.g. a
    /// development version: then nothing is offered).
    #[must_use]
    pub fn version_code(&self) -> Option<u32> {
        assets::version_code(&self.version)
    }

    /// The checked APK, downloaded first if needed.
    ///
    /// # Errors
    /// See [`ApkError`].
    pub async fn get(&self) -> Result<Apk, ApkError> {
        let key = self.key.as_ref().ok_or(ApkError::NoKey)?;
        let version_code = self.version_code().ok_or_else(|| ApkError::NotPublished {
            version: self.version.clone(),
        })?;
        let _one_at_a_time = self.lock.lock().await;
        let apk_name = assets::apk(&self.version);
        let paths = Files::new(&self.dir, &apk_name);
        if check(key, &paths, &apk_name, &self.version).is_ok() {
            return Ok(self.apk(paths.apk, version_code));
        }
        self.download(&paths, &apk_name).await?;
        // Never keep what failed: the next try downloads it again.
        check(key, &paths, &apk_name, &self.version).inspect_err(|_| paths.remove())?;
        tracing::info!(version = %self.version, "tablet APK downloaded and checked");
        Ok(self.apk(paths.apk, version_code))
    }

    fn apk(&self, path: PathBuf, version_code: u32) -> Apk {
        Apk {
            version: self.version.clone(),
            version_code,
            path,
        }
    }

    async fn download(&self, paths: &Files, apk_name: &str) -> Result<(), ApkError> {
        let http = self.http.as_ref().ok_or_else(|| ApkError::Network {
            detail: "no HTTP client".into(),
        })?;
        std::fs::create_dir_all(&self.dir).map_err(storage)?;
        // Older versions' APKs are of no use any more.
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("padboard-") && name.ends_with(".apk") && name != apk_name {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        for (file, path) in [
            (CHECKSUMS, &paths.sums),
            (CHECKSUMS_SIGNATURE, &paths.signature),
            (apk_name, &paths.apk),
        ] {
            let url = assets::download_url(&self.base_url, &self.version, file);
            let bytes = fetch(http, &url, &self.version).await?;
            write_atomically(path, &bytes)?;
        }
        Ok(())
    }
}

/// Where the files of one version are kept.
struct Files {
    sums: PathBuf,
    signature: PathBuf,
    apk: PathBuf,
}

impl Files {
    fn new(dir: &Path, apk_name: &str) -> Self {
        Self {
            sums: dir.join(CHECKSUMS),
            signature: dir.join(CHECKSUMS_SIGNATURE),
            apk: dir.join(apk_name),
        }
    }

    fn remove(&self) {
        for path in [&self.sums, &self.signature, &self.apk] {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The files on disk are the release's: signed sums, matching APK.
fn check(key: &UpdateKey, paths: &Files, apk_name: &str, version: &str) -> Result<(), ApkError> {
    let read = |path: &Path| std::fs::read(path).map_err(storage);
    let sums = read(&paths.sums)?;
    let signature =
        String::from_utf8(read(&paths.signature)?).map_err(|e| ApkError::Signature {
            detail: e.to_string(),
        })?;
    let sums = Checksums::verify(key, &sums, &signature, version).map_err(signature_error)?;
    sums.check(apk_name, &read(&paths.apk)?)
        .map_err(signature_error)
}

async fn fetch(http: &reqwest::Client, url: &str, version: &str) -> Result<Vec<u8>, ApkError> {
    let network = |e: reqwest::Error| ApkError::Network {
        detail: e.to_string(),
    };
    let response = http.get(url).send().await.map_err(network)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApkError::NotPublished {
            version: version.to_owned(),
        });
    }
    if !response.status().is_success() {
        return Err(ApkError::Network {
            detail: format!("{url}: {}", response.status()),
        });
    }
    let bytes = response.bytes().await.map_err(network)?;
    if bytes.len() > padboard_protocol::MAX_UPDATE_SIZE as usize {
        return Err(ApkError::Signature {
            detail: format!("{url} is too large"),
        });
    }
    Ok(bytes.to_vec())
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), ApkError> {
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes).map_err(storage)?;
    std::fs::rename(&partial, path).map_err(storage)
}

fn storage(e: std::io::Error) -> ApkError {
    ApkError::Storage {
        detail: e.to_string(),
    }
}

fn signature_error(e: SigningError) -> ApkError {
    ApkError::Signature {
        detail: e.to_string(),
    }
}

/// Shared by the commands of the app.
pub type SharedApks = Arc<Apks>;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    const KEY: &str = include_str!("fixtures/test-key.pub");
    const SUMS: &[u8] = include_bytes!("fixtures/SHA256SUMS");
    const SIG: &[u8] = include_bytes!("fixtures/SHA256SUMS.sig");
    const APK: &[u8] = b"not a real apk, version 1.2.3\n";

    type Served = Arc<Mutex<HashMap<String, Vec<u8>>>>;

    /// A release server: `/download/v1.2.3/<file>` from `files`, 404 else.
    async fn release_server(files: Served) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/releases", listener.local_addr().unwrap());
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut buffer = vec![0u8; 4096];
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..n]).into_owned();
                let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
                let name = path.rsplit('/').next().unwrap_or_default().to_owned();
                let body = files.lock().unwrap().get(&name).cloned();
                let head = match &body {
                    Some(b) => format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        b.len()
                    ),
                    None => {
                        "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                            .to_owned()
                    }
                };
                let _ = socket.write_all(head.as_bytes()).await;
                if let Some(body) = body {
                    let _ = socket.write_all(&body).await;
                }
            }
        });
        (base, hits)
    }

    fn genuine() -> Served {
        Arc::new(Mutex::new(HashMap::from([
            ("SHA256SUMS".to_owned(), SUMS.to_vec()),
            ("SHA256SUMS.sig".to_owned(), SIG.to_vec()),
            ("padboard-1.2.3.apk".to_owned(), APK.to_vec()),
        ])))
    }

    fn apks(base: String, dir: &Path, version: &str) -> Apks {
        Apks::with(
            Some(UpdateKey::from_tauri(KEY).unwrap()),
            base,
            dir.to_path_buf(),
            version.to_owned(),
        )
    }

    #[tokio::test]
    async fn the_signed_apk_is_downloaded_once_then_reused() {
        let (base, hits) = release_server(genuine()).await;
        let tmp = tempfile::tempdir().unwrap();
        let apks = apks(base, tmp.path(), "1.2.3");
        let apk = apks.get().await.unwrap();
        assert_eq!(apk.version_code, 1_020_399);
        assert_eq!(std::fs::read(&apk.path).unwrap(), APK);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3);
        apks.get().await.unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3, "cached");
    }

    #[tokio::test]
    async fn a_modified_apk_is_refused_and_not_kept() {
        let served = genuine();
        served
            .lock()
            .unwrap()
            .insert("padboard-1.2.3.apk".into(), b"evil".to_vec());
        let (base, _) = release_server(served).await;
        let tmp = tempfile::tempdir().unwrap();
        let apks = apks(base, tmp.path(), "1.2.3");
        assert!(matches!(apks.get().await, Err(ApkError::Signature { .. })));
        assert!(!tmp.path().join("padboard-1.2.3.apk").exists());
    }

    #[tokio::test]
    async fn a_tampered_cache_is_downloaded_again() {
        let (base, hits) = release_server(genuine()).await;
        let tmp = tempfile::tempdir().unwrap();
        let apks = apks(base, tmp.path(), "1.2.3");
        let apk = apks.get().await.unwrap();
        std::fs::write(&apk.path, b"changed on disk").unwrap();
        let again = apks.get().await.unwrap();
        assert_eq!(std::fs::read(&again.path).unwrap(), APK);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 6);
    }

    #[tokio::test]
    async fn checksums_of_another_version_are_refused() {
        // The genuine 1.2.3 files served as version 1.2.4's.
        let served = genuine();
        let apk = served.lock().unwrap().remove("padboard-1.2.3.apk").unwrap();
        served
            .lock()
            .unwrap()
            .insert("padboard-1.2.4.apk".into(), apk);
        let (base, _) = release_server(served).await;
        let tmp = tempfile::tempdir().unwrap();
        let apks = apks(base, tmp.path(), "1.2.4");
        let Err(ApkError::Signature { detail }) = apks.get().await else {
            panic!("expected a signature error");
        };
        assert!(detail.contains("version"), "{detail}");
    }

    #[tokio::test]
    async fn an_unpublished_version_and_no_key() {
        let (base, _) = release_server(Arc::new(Mutex::new(HashMap::new()))).await;
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            apks(base.clone(), tmp.path(), "1.2.3").get().await,
            Err(ApkError::NotPublished {
                version: "1.2.3".into()
            })
        );
        assert_eq!(
            apks(base.clone(), tmp.path(), "0.2.0-dev").get().await,
            Err(ApkError::NotPublished {
                version: "0.2.0-dev".into()
            })
        );
        let unsigned = Apks::with(None, base, tmp.path().to_path_buf(), "1.2.3".into());
        assert_eq!(unsigned.get().await, Err(ApkError::NoKey));
    }

    #[tokio::test]
    async fn older_apks_are_removed() {
        let (base, _) = release_server(genuine()).await;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("padboard-1.2.2.apk"), b"old").unwrap();
        apks(base, tmp.path(), "1.2.3").get().await.unwrap();
        assert!(!tmp.path().join("padboard-1.2.2.apk").exists());
    }
}
