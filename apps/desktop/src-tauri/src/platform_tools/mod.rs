//! Google's Android SDK Platform-Tools (`adb`) on Windows, downloaded on the
//! user's request ("Installer adb", USB card), never shipped with Padboard.
//!
//! **An exception to the free software rule** (decided for v0.2): these
//! binaries come under the Android SDK License Agreement, which forbids
//! redistributing them (§3.4) and only grants their use "to develop
//! applications" (§3.1). So Padboard does not bundle them: the user reads a
//! summary, accepts the license, and the app downloads them from Google.
//! USB is optional (Wi-Fi works without adb), and Linux uses the free `adb`
//! package of the distribution (a dependency of the `.deb`).
//!
//! What is downloaded is pinned: one version, its URL and SHA-256 (from
//! Google's repository index, `scripts/platform-tools-pin.sh`). Each `.exe`
//! and `.dll` must also carry a valid Authenticode signature by "Google LLC".
//! Extracted to `%LOCALAPPDATA%\io.github.hugoambiehl.padboard\platform-tools`,
//! found by [`crate::adb::locate`] after `PATH` (an `adb` the user installed
//! wins: two adb servers of different versions fight over the port).

#[cfg(windows)]
mod authenticode;

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

/// The version downloaded.
pub const VERSION: &str = "37.0.3";
/// Google's archive of that version (`repository2-3.xml`, Windows).
pub const URL: &str = "https://dl.google.com/android/repository/platform-tools_r37.0.3-win.zip";
/// Its SHA-256 (Google publishes a SHA-1, checked when pinning).
pub const SHA256: &str = "b5d2015f5d5b7a24a2ac646a245f20f63fb501e4174c61863c74bed1df6de4bd";
/// Its size, in bytes; a download above twice that is cut short.
pub const SIZE: u64 = 8_188_595;
/// The license the user accepts.
pub const LICENSE_URL: &str = "https://developer.android.com/studio/terms";
/// Google's page of the Platform-Tools (to download them by hand).
pub const PAGE_URL: &str = "https://developer.android.com/tools/releases/platform-tools";
/// Who must have signed every executable of the archive.
pub const SIGNER: &str = "Google LLC";
/// The folder of the archive holding the tools.
const ARCHIVE_DIR: &str = "platform-tools/";

/// The archive to download (the pinned one, or a test one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl Archive {
    #[must_use]
    pub fn pinned() -> Self {
        Self {
            url: URL.to_owned(),
            sha256: SHA256.to_owned(),
            size: SIZE,
        }
    }
}

/// Why the tools were not installed (codes translated by the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlatformToolsError {
    /// Only offered on Windows.
    #[error("not offered on this system")]
    Unsupported,
    /// The license was not accepted.
    #[error("license not accepted")]
    NotAccepted,
    /// Google no longer serves the pinned version (they remove old ones).
    #[error("the pinned version is no longer available (HTTP {status})")]
    Unavailable { status: u16 },
    /// No connection, or cut.
    #[error("download failed: {detail}")]
    Network { detail: String },
    /// Not the pinned file.
    #[error("the download does not match its SHA-256")]
    Checksum,
    /// An executable is not validly signed by Google.
    #[error("{file} is not signed by {SIGNER}: {detail}")]
    Signature { file: String, detail: String },
    /// Unreadable archive, or a file that cannot be written.
    #[error("cannot install: {detail}")]
    Install { detail: String },
}

/// What the USB card shows about the downloadable tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformToolsInfo {
    /// "Installer adb" can be offered (Windows).
    pub offered: bool,
    pub version: &'static str,
    /// `adb` of the downloaded copy, when there is one.
    pub installed: Option<String>,
}

/// Where the tools are extracted (Windows only).
#[must_use]
pub fn install_dir() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    std::env::var_os("LOCALAPPDATA").map(|dir| {
        PathBuf::from(dir)
            .join("io.github.hugoambiehl.padboard")
            .join("platform-tools")
    })
}

/// `adb` of the downloaded copy, if present.
#[must_use]
pub fn installed_adb() -> Option<PathBuf> {
    install_dir()
        .map(|dir| dir.join("adb.exe"))
        .filter(|adb| adb.is_file())
}

#[must_use]
pub fn info() -> PlatformToolsInfo {
    PlatformToolsInfo {
        offered: cfg!(windows),
        version: VERSION,
        installed: installed_adb().map(|adb| adb.display().to_string()),
    }
}

/// Downloads `archive`, checks its SHA-256, extracts its `platform-tools/`
/// folder into `dir` (replacing what was there only once everything is
/// checked), and checks each executable with `verify` (Authenticode on
/// Windows). Returns the path of `adb`.
///
/// # Errors
/// See [`PlatformToolsError`]; nothing is left behind on failure.
pub async fn install(
    http: &reqwest::Client,
    archive: &Archive,
    dir: &Path,
    verify: impl Fn(&Path) -> Result<(), String>,
) -> Result<PathBuf, PlatformToolsError> {
    let bytes = download(http, archive).await?;
    let digest = Sha256::digest(&bytes);
    if !hex(&digest).eq_ignore_ascii_case(&archive.sha256) {
        tracing::error!("platform-tools: SHA-256 mismatch");
        return Err(PlatformToolsError::Checksum);
    }
    let staging = sibling(dir, "new");
    let result = extract(&bytes, &staging).and_then(|()| check_executables(&staging, &verify));
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    replace(&staging, dir)?;
    let adb = dir.join("adb.exe");
    tracing::info!(version = VERSION, path = %adb.display(), "platform-tools installed");
    Ok(adb)
}

/// Removes the downloaded copy (its adb server must be stopped first:
/// Windows keeps a running program's file locked).
///
/// # Errors
/// The folder cannot be removed.
pub fn remove(dir: &Path) -> Result<(), PlatformToolsError> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {
            tracing::info!(path = %dir.display(), "platform-tools removed");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PlatformToolsError::Install {
            detail: e.to_string(),
        }),
    }
}

async fn download(
    http: &reqwest::Client,
    archive: &Archive,
) -> Result<Vec<u8>, PlatformToolsError> {
    let network = |e: reqwest::Error| PlatformToolsError::Network {
        detail: e.to_string(),
    };
    let mut response = http.get(&archive.url).send().await.map_err(network)?;
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(%status, url = %archive.url, "platform-tools download refused");
        return Err(if status.is_client_error() {
            PlatformToolsError::Unavailable {
                status: status.as_u16(),
            }
        } else {
            PlatformToolsError::Network {
                detail: format!("HTTP {status}"),
            }
        });
    }
    let limit = archive.size.saturating_mul(2);
    let mut bytes = Vec::with_capacity(usize::try_from(archive.size).unwrap_or(0));
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > limit {
            return Err(PlatformToolsError::Checksum);
        }
    }
    Ok(bytes)
}

/// Extracts the files under `platform-tools/` into `to` (created afresh).
fn extract(bytes: &[u8], to: &Path) -> Result<(), PlatformToolsError> {
    let failed = |detail: String| PlatformToolsError::Install { detail };
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).map_err(|e| failed(e.to_string()))?;
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| failed(e.to_string()))?;
    let mut found_adb = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| failed(e.to_string()))?;
        // `enclosed_name` refuses absolute paths and `..` (zip slip).
        let Some(name) = entry.enclosed_name() else {
            return Err(failed(format!("unsafe path {:?}", entry.name())));
        };
        let Ok(relative) = name.strip_prefix(ARCHIVE_DIR) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = to.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| failed(e.to_string()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| failed(e.to_string()))?;
        }
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| failed(e.to_string()))?;
        std::fs::write(&target, content).map_err(|e| failed(e.to_string()))?;
        found_adb |= relative == Path::new("adb.exe") || relative == Path::new("adb");
    }
    if found_adb {
        Ok(())
    } else {
        Err(failed("no adb in the archive".into()))
    }
}

/// Every `.exe` and `.dll` extracted must pass `verify`.
fn check_executables(
    dir: &Path,
    verify: &impl Fn(&Path) -> Result<(), String>,
) -> Result<(), PlatformToolsError> {
    let entries = std::fs::read_dir(dir).map_err(|e| PlatformToolsError::Install {
        detail: e.to_string(),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let executable = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("dll"));
        if executable && let Err(detail) = verify(&path) {
            let file = entry.file_name().to_string_lossy().into_owned();
            tracing::error!(%file, %detail, "platform-tools: signature refused");
            return Err(PlatformToolsError::Signature { file, detail });
        }
    }
    Ok(())
}

/// Puts `staging` in place of `dir` (the old copy removed first).
fn replace(staging: &Path, dir: &Path) -> Result<(), PlatformToolsError> {
    let failed = |e: std::io::Error| {
        let _ = std::fs::remove_dir_all(staging);
        PlatformToolsError::Install {
            detail: e.to_string(),
        }
    };
    if dir.exists() {
        let old = sibling(dir, "old");
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(dir, &old).map_err(failed)?;
        let _ = std::fs::remove_dir_all(&old);
    }
    std::fs::rename(staging, dir).map_err(failed)
}

/// `<dir>.<suffix>`, next to `dir`.
fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    dir.with_file_name(name)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// The real check of an extracted executable: a valid Authenticode
/// signature by [`SIGNER`].
///
/// # Errors
/// Not signed, untrusted, or signed by someone else.
#[cfg(windows)]
pub fn verify_signature(path: &Path) -> Result<(), String> {
    let signer = authenticode::signer(path)?;
    if signer == SIGNER {
        Ok(())
    } else {
        Err(format!("signed by {signer:?}"))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    /// A small archive shaped like Google's.
    fn archive_bytes(extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.add_directory("platform-tools/", options).unwrap();
        for (name, content) in [
            ("platform-tools/adb.exe", b"adb".as_slice()),
            ("platform-tools/AdbWinApi.dll", b"dll"),
            ("platform-tools/NOTICE.txt", b"notices"),
            ("other/ignored.txt", b"x"),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            zip.start_file(name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// Serves `body` with `status` to every request.
    async fn serve(status: u16, body: Vec<u8>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/platform-tools.zip",
            listener.local_addr().unwrap()
        );
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            }
        });
        url
    }

    fn archive(url: String, bytes: &[u8]) -> Archive {
        Archive {
            url,
            sha256: hex(&Sha256::digest(bytes)),
            size: bytes.len() as u64,
        }
    }

    fn http() -> reqwest::Client {
        crate::updates::client().unwrap()
    }

    #[tokio::test]
    async fn installs_the_checked_archive_and_replaces_the_old_copy() {
        let bytes = archive_bytes(&[]);
        let url = serve(200, bytes.clone()).await;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("platform-tools");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("stale.txt"), "old").unwrap();

        let checked = std::sync::Mutex::new(Vec::new());
        let adb = install(&http(), &archive(url, &bytes), &dir, |path| {
            checked
                .lock()
                .unwrap()
                .push(path.file_name().unwrap().to_owned());
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(std::fs::read(&adb).unwrap(), b"adb");
        assert_eq!(std::fs::read(dir.join("NOTICE.txt")).unwrap(), b"notices");
        assert!(!dir.join("stale.txt").exists());
        assert!(!tmp.path().join("ignored.txt").exists());
        let mut checked = checked.into_inner().unwrap();
        checked.sort();
        assert_eq!(checked, ["AdbWinApi.dll", "adb.exe"]);
        assert_eq!(
            std::fs::read_dir(tmp.path()).unwrap().count(),
            1,
            "no staging folder left"
        );
    }

    #[tokio::test]
    async fn another_file_is_refused_and_the_old_copy_kept() {
        let bytes = archive_bytes(&[]);
        let url = serve(200, archive_bytes(&[("platform-tools/evil.exe", b"x")])).await;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("platform-tools");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("adb.exe"), "old").unwrap();

        let result = install(&http(), &archive(url, &bytes), &dir, |_| Ok(())).await;
        assert_eq!(result, Err(PlatformToolsError::Checksum));
        assert_eq!(std::fs::read(dir.join("adb.exe")).unwrap(), b"old");
    }

    #[tokio::test]
    async fn an_unsigned_executable_is_refused() {
        let bytes = archive_bytes(&[]);
        let url = serve(200, bytes.clone()).await;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("platform-tools");

        let result = install(&http(), &archive(url, &bytes), &dir, |path| {
            if path.ends_with("AdbWinApi.dll") {
                Err("signed by \"Someone\"".into())
            } else {
                Ok(())
            }
        })
        .await;
        assert_eq!(
            result,
            Err(PlatformToolsError::Signature {
                file: "AdbWinApi.dll".into(),
                detail: "signed by \"Someone\"".into()
            })
        );
        assert!(!dir.exists());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    /// Google removed the pinned version: said so, so the UI can offer the
    /// manual choice of adb.
    #[tokio::test]
    async fn a_version_google_removed_is_reported() {
        let url = serve(404, b"gone".to_vec()).await;
        let tmp = tempfile::tempdir().unwrap();
        let result = install(
            &http(),
            &archive(url, b"anything"),
            &tmp.path().join("platform-tools"),
            |_| Ok(()),
        )
        .await;
        assert_eq!(result, Err(PlatformToolsError::Unavailable { status: 404 }));
    }

    #[tokio::test]
    async fn unsafe_paths_and_archives_without_adb_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("platform-tools/../../escape.txt", options)
            .unwrap();
        zip.write_all(b"x").unwrap();
        let evil = zip.finish().unwrap().into_inner();
        assert!(matches!(
            extract(&evil, &tmp.path().join("a")),
            Err(PlatformToolsError::Install { .. })
        ));
        assert!(!tmp.path().join("escape.txt").exists());

        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("platform-tools/fastboot.exe", options)
            .unwrap();
        let no_adb = zip.finish().unwrap().into_inner();
        assert!(matches!(
            extract(&no_adb, &tmp.path().join("b")),
            Err(PlatformToolsError::Install { .. })
        ));
    }

    #[test]
    fn the_pin_is_consistent() {
        assert!(URL.contains(&format!("platform-tools_r{VERSION}-win.zip")));
        assert!(URL.starts_with("https://dl.google.com/android/repository/"));
        assert_eq!(SHA256.len(), 64);
        assert!(SHA256.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(
            sibling(Path::new("/a/platform-tools"), "new"),
            Path::new("/a/platform-tools.new")
        );
        // Only offered on Windows.
        assert_eq!(info().offered, cfg!(windows));
        if !cfg!(windows) {
            assert_eq!(install_dir(), None);
        }
    }
}
