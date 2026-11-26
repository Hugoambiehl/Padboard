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
//! Fallback (#18): Google republishes an archive under the same version
//! (37.0.3 on 2026-11-19), so a whole zip that is not the pinned one is
//! accepted when Google's index lists it for the pinned version, with the
//! same size and SHA-1; the signatures are still checked, and the log, the
//! dialog and the USB card say the fallback was used.
//! Extracted to `%LOCALAPPDATA%\io.github.hugoambiehl.padboard\platform-tools`,
//! found by [`crate::adb::locate`] after `PATH` (an `adb` the user installed
//! wins: two adb servers of different versions fight over the port).

#[cfg(windows)]
pub(crate) mod authenticode;
mod server_owner;

pub use server_owner::{same_program, server_owner};

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest as _, Sha256};

/// The version downloaded.
pub const VERSION: &str = "37.0.3";
/// Google's archive of that version (`repository2-3.xml`, Windows).
pub const URL: &str = "https://dl.google.com/android/repository/platform-tools_r37.0.3-win.zip";
/// Its SHA-256 (Google publishes a SHA-1, checked when pinning).
pub const SHA256: &str = "89d108360a3bf2924d3bea837239edaccc22fd6ff5befd3787c4348a9fe4f09d";
/// Its size, in bytes; a download above twice that is cut short.
pub const SIZE: u64 = 8_188_601;
/// The license the user accepts.
pub const LICENSE_URL: &str = "https://developer.android.com/studio/terms";
/// Google's page of the Platform-Tools (to download them by hand).
pub const PAGE_URL: &str = "https://developer.android.com/tools/releases/platform-tools";
/// Who must have signed every executable of the archive.
pub const SIGNER: &str = "Google LLC";
/// The folder of the archive holding the tools.
const ARCHIVE_DIR: &str = "platform-tools/";
/// Google's repository index (what Android Studio's SDK manager reads): the
/// fallback when the archive is not the pinned one (#18).
pub const INDEX_URL: &str = "https://dl.google.com/android/repository/repository2-3.xml";
/// The index is about 430 kB; anything far larger is not it.
const INDEX_LIMIT: usize = 8 << 20;
/// Written into the installed folder when the archive was accepted from the
/// index, read by [`info`].
const FROM_INDEX_FILE: &str = "padboard-from-index.json";

/// The archive to download (the pinned one, or a test one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    /// Google's index, asked when the download is another archive.
    pub index_url: String,
}

impl Archive {
    #[must_use]
    pub fn pinned() -> Self {
        Self {
            version: VERSION.to_owned(),
            url: URL.to_owned(),
            sha256: SHA256.to_owned(),
            size: SIZE,
            index_url: INDEX_URL.to_owned(),
        }
    }

    /// The archive's file name, as the index names it.
    fn file_name(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or_default()
    }
}

/// An archive accepted from Google's index instead of the pin (#18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FromIndex {
    pub version: String,
    pub size: u64,
    pub sha1: String,
    pub sha256: String,
    pub pinned_sha256: String,
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
    /// Not the pinned file (details in the log).
    #[error("the download is not the pinned archive ({reason:?}, {received} of {expected} bytes)")]
    Checksum {
        reason: Mismatch,
        received: u64,
        expected: u64,
    },
    /// An executable is not validly signed by Google.
    #[error("{file} is not signed by {SIGNER}: {detail}")]
    Signature { file: String, detail: String },
    /// Unreadable archive, or a file that cannot be written.
    #[error("cannot install: {detail}")]
    Install { detail: String },
}

/// Why a download is not the pinned archive, guessed from what was received
/// so the user is told what to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mismatch {
    /// A web page instead of the archive: a Wi-Fi portal, a proxy, a filter.
    Html,
    /// The transfer was cut: fewer bytes than the archive, or a network error
    /// on the way.
    Truncated,
    /// A whole zip archive, but not the pinned one: Google republished the
    /// file under the same version (37.0.3, 2026-11-19). A new Padboard
    /// version must pin it.
    Changed,
    /// Anything else.
    Unexpected,
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
    /// That copy is not the pinned archive: Google republished it, and it
    /// was accepted from Google's index (#18).
    pub from_index: Option<FromIndex>,
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
    let installed = installed_adb();
    PlatformToolsInfo {
        offered: cfg!(windows),
        version: VERSION,
        from_index: installed
            .as_ref()
            .and_then(|adb| adb.parent())
            .and_then(read_from_index),
        installed: installed.map(|adb| adb.display().to_string()),
    }
}

/// How the copy in `dir` was accepted, when it came from the index.
fn read_from_index(dir: &Path) -> Option<FromIndex> {
    let text = std::fs::read_to_string(dir.join(FROM_INDEX_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Downloads `archive`, checks its SHA-256 (or, for another whole zip, its
/// size and SHA-1 against Google's index, [`check_index`]), extracts its
/// `platform-tools/` folder into `dir` (replacing what was there only once
/// everything is checked), and checks each executable with `verify`
/// (Authenticode on Windows). Returns the path of `adb`, and how the
/// archive was accepted when the index was used.
///
/// # Errors
/// See [`PlatformToolsError`]; nothing is left behind on failure.
pub async fn install(
    http: &reqwest::Client,
    archive: &Archive,
    dir: &Path,
    verify: impl Fn(&Path) -> Result<(), String>,
) -> Result<(PathBuf, Option<FromIndex>), PlatformToolsError> {
    let received = download(http, archive).await?;
    let sha256 = hex(&Sha256::digest(&received.bytes));
    let mut from_index = None;
    if received.cut.is_some() || !sha256.eq_ignore_ascii_case(&archive.sha256) {
        let reason = mismatch(&received, archive.size);
        let index = if reason == Mismatch::Changed {
            check_index(http, archive, &received.bytes).await
        } else {
            Err("not a whole zip archive: Google's index not asked".to_owned())
        };
        match index {
            Ok(entry) => {
                tracing::warn!(
                    version = %entry.version,
                    size = entry.size,
                    sha1 = %entry.sha1,
                    %sha256,
                    pinned_sha256 = %archive.sha256,
                    url = %received.final_url,
                    "platform-tools: NOT the pinned archive, accepted from Google's index \
                     (same version, size and SHA-1: Google republished it); \
                     every executable's signature is checked next"
                );
                from_index = Some(entry);
            }
            Err(index) => {
                tracing::error!(
                    ?reason,
                    received = received.bytes.len(),
                    expected = archive.size,
                    %sha256,
                    expected_sha256 = %archive.sha256,
                    status = %received.status,
                    content_type = received.content_type.as_deref().unwrap_or("-"),
                    url = %received.final_url,
                    cut = received.cut.as_deref().unwrap_or("-"),
                    start = %preview(&received.bytes),
                    %index,
                    "platform-tools: the download is not the pinned archive"
                );
                return Err(PlatformToolsError::Checksum {
                    reason,
                    received: received.bytes.len() as u64,
                    expected: archive.size,
                });
            }
        }
    }
    let bytes = received.bytes;
    let staging = sibling(dir, "new");
    let result = extract(&bytes, &staging)
        .and_then(|()| check_executables(&staging, &verify))
        .and_then(|()| record_from_index(&staging, from_index.as_ref()));
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    replace(&staging, dir)?;
    let adb = dir.join("adb.exe");
    if from_index.is_some() {
        tracing::warn!(
            version = %archive.version,
            path = %adb.display(),
            "platform-tools installed from Google's index, not the pinned archive (signatures checked)"
        );
    } else {
        tracing::info!(version = %archive.version, path = %adb.display(), "platform-tools installed");
    }
    Ok((adb, from_index))
}

/// Keeps how the archive was accepted, next to the tools ([`info`]).
fn record_from_index(dir: &Path, entry: Option<&FromIndex>) -> Result<(), PlatformToolsError> {
    let Some(entry) = entry else { return Ok(()) };
    let json = serde_json::to_string_pretty(entry).map_err(|e| PlatformToolsError::Install {
        detail: e.to_string(),
    })?;
    std::fs::write(dir.join(FROM_INDEX_FILE), json).map_err(|e| PlatformToolsError::Install {
        detail: e.to_string(),
    })
}

/// The fallback for a republished archive (#18): `bytes` is accepted when
/// Google's index lists, for the pinned version, the pinned file name with
/// exactly this size and SHA-1. Never another version: a newer one is
/// never taken without a new Padboard release. SHA-1 alone is not
/// collision-resistant: the Authenticode check of every executable, which
/// follows, is the real guard.
///
/// # Errors
/// Why the archive is not accepted (for the log).
async fn check_index(
    http: &reqwest::Client,
    archive: &Archive,
    bytes: &[u8],
) -> Result<FromIndex, String> {
    let index = fetch_index(http, &archive.index_url).await?;
    let Some(entry) = windows_archive(&index, &archive.version)? else {
        return Err(format!(
            "Google's index has no Windows archive of {}",
            archive.version
        ));
    };
    let sha1 = hex(&Sha1::digest(bytes));
    let sha256 = hex(&Sha256::digest(bytes));
    if entry.url != archive.file_name() {
        return Err(format!(
            "the index names {}, not {}",
            entry.url,
            archive.file_name()
        ));
    }
    if entry.size != bytes.len() as u64 {
        return Err(format!(
            "the index says {} bytes, {} received",
            entry.size,
            bytes.len()
        ));
    }
    if !entry.sha1.eq_ignore_ascii_case(&sha1) {
        return Err(format!(
            "the index's SHA-1 is {}, the download's {sha1}",
            entry.sha1
        ));
    }
    Ok(FromIndex {
        version: archive.version.clone(),
        size: entry.size,
        sha1,
        sha256,
        pinned_sha256: archive.sha256.clone(),
    })
}

async fn fetch_index(http: &reqwest::Client, url: &str) -> Result<String, String> {
    let mut response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Google's index unreachable: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Google's index answered HTTP {}",
            response.status()
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Google's index cut: {e}"))?
    {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > INDEX_LIMIT {
            return Err(format!("Google's index is over {INDEX_LIMIT} bytes"));
        }
    }
    String::from_utf8(bytes).map_err(|_| "Google's index is not UTF-8".to_owned())
}

/// One archive of the index.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct IndexArchive {
    url: String,
    size: u64,
    sha1: String,
    host_os: String,
}

/// The Windows archive of the `platform-tools` package at `version` in
/// Google's `repository2-3.xml`, if listed.
///
/// # Errors
/// The index is not readable XML.
fn windows_archive(xml: &str, version: &str) -> Result<Option<IndexArchive>, String> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // Element names from the root, and the state of the package being read.
    let mut path: Vec<String> = Vec::new();
    let mut in_package = false;
    let mut revision = [String::new(), String::new(), String::new()];
    let mut archive = IndexArchive::default();
    let mut sha1_checksum = false;
    let mut found = Vec::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("Google's index is not readable XML: {e}"))?;
        match event {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_owned();
                if name == "remotePackage" {
                    in_package = e
                        .try_get_attribute("path")
                        .ok()
                        .flatten()
                        .is_some_and(|a| a.value.as_ref() == "platform-tools");
                    revision = Default::default();
                    found.clear();
                } else if in_package && name == "archive" {
                    archive = IndexArchive::default();
                } else if in_package && name == "checksum" {
                    sha1_checksum = e
                        .try_get_attribute("type")
                        .ok()
                        .flatten()
                        .is_some_and(|a| a.value.as_ref() == "sha1");
                }
                path.push(name);
            }
            Event::Text(text) if in_package => {
                let text = text.xml10_content().trim().to_owned();
                match path.last().map(String::as_str) {
                    Some("major") => revision[0] = text,
                    Some("minor") => revision[1] = text,
                    Some("micro") => revision[2] = text,
                    Some("size") => archive.size = text.parse().unwrap_or(0),
                    Some("checksum") if sha1_checksum => archive.sha1 = text,
                    Some("url") => archive.url = text,
                    Some("host-os") => archive.host_os = text,
                    _ => {}
                }
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                if in_package && name == "archive" {
                    found.push(std::mem::take(&mut archive));
                } else if in_package && name == "remotePackage" {
                    in_package = false;
                    let listed = revision
                        .iter()
                        .map(|part| if part.is_empty() { "0" } else { part });
                    if listed.collect::<Vec<_>>().join(".") == version {
                        return Ok(found.drain(..).find(|a| a.host_os == "windows"));
                    }
                }
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
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

/// What the server sent, kept to explain a refused download in the log.
struct Received {
    bytes: Vec<u8>,
    status: reqwest::StatusCode,
    content_type: Option<String>,
    /// After redirections.
    final_url: String,
    /// Why the transfer stopped early: a network error after some bytes, or
    /// the size limit.
    cut: Option<String>,
}

async fn download(
    http: &reqwest::Client,
    archive: &Archive,
) -> Result<Received, PlatformToolsError> {
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
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned());
    let final_url = response.url().to_string();
    let limit = archive.size.saturating_mul(2);
    let mut bytes = Vec::with_capacity(usize::try_from(archive.size).unwrap_or(0));
    let mut cut = None;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                bytes.extend_from_slice(&chunk);
                if bytes.len() as u64 > limit {
                    cut = Some(format!("more than {limit} bytes"));
                    break;
                }
            }
            Ok(None) => break,
            Err(e) if bytes.is_empty() => return Err(network(e)),
            Err(e) => {
                cut = Some(e.to_string());
                break;
            }
        }
    }
    Ok(Received {
        bytes,
        status,
        content_type,
        final_url,
        cut,
    })
}

/// Guesses why `received` is not the pinned archive of `expected` bytes.
fn mismatch(received: &Received, expected: u64) -> Mismatch {
    let bytes = &received.bytes;
    let html_type = received
        .content_type
        .as_deref()
        .is_some_and(|t| t.trim_start().to_ascii_lowercase().starts_with("text/html"));
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let starts_with_tag = text.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<');
    if html_type || starts_with_tag {
        Mismatch::Html
    } else if received.cut.is_some() {
        Mismatch::Truncated
    } else if bytes.starts_with(b"PK\x03\x04")
        && zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).is_ok()
    {
        Mismatch::Changed
    } else if (bytes.len() as u64) < expected {
        Mismatch::Truncated
    } else {
        Mismatch::Unexpected
    }
}

/// The first bytes of a download, for the log: in hex, followed by the text
/// when they read as text (a portal's page often says who sent it).
fn preview(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(16)];
    let mut out = hex(head);
    if !head.is_empty()
        && head
            .iter()
            .all(|b| b.is_ascii_graphic() || b.is_ascii_whitespace() || *b >= 0x80)
    {
        let text = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!(" {text:?}"));
    }
    out
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
        let length = body.len();
        serve_with(status, "", body, length).await
    }

    /// Serves `body` with `status`, extra `headers` (`name: value\r\n`) and a
    /// declared `length` (above the body's: the transfer is cut).
    async fn serve_with(
        status: u16,
        headers: &'static str,
        body: Vec<u8>,
        length: usize,
    ) -> String {
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
                    "HTTP/1.1 {status} X\r\ncontent-length: {length}\r\n{headers}connection: close\r\n\r\n"
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            }
        });
        url
    }

    /// The pinned `bytes` at `url`; Google's index unreachable (port 9).
    fn archive(url: String, bytes: &[u8]) -> Archive {
        Archive {
            version: "37.0.3".into(),
            url,
            sha256: hex(&Sha256::digest(bytes)),
            size: bytes.len() as u64,
            index_url: "http://127.0.0.1:9/repository2-3.xml".into(),
        }
    }

    /// Serves each `(path, body)` with 200, anything else with 404; returns
    /// the server's base URL.
    async fn serve_routes(routes: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = [0u8; 4096];
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let (status, body) = routes
                    .iter()
                    .find(|(route, _)| *route == path)
                    .map_or((404, b"not found".as_slice()), |(_, body)| {
                        (200, body.as_slice())
                    });
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body).await;
            }
        });
        base
    }

    /// An index shaped like Google's `repository2-3.xml`: another package
    /// first (with a Windows archive of its own), then the platform-tools
    /// at `version`, whose Windows archive is `name`, `size` and `sha1`.
    fn index(version: &str, name: &str, size: usize, sha1: &str) -> String {
        let [major, minor, micro] = {
            let parts: Vec<&str> = version.split('.').collect();
            [parts[0], parts[1], parts[2]]
        };
        format!(
            r#"<?xml version='1.0' encoding='utf-8'?>
<sdk:sdk-repository xmlns:sdk="http://schemas.android.com/sdk/android/repo/repository2/03" xmlns:common="http://schemas.android.com/repository/android/common/02" xmlns:generic="http://schemas.android.com/repository/android/generic/02" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <license id="android-sdk-license" type="text">Terms &amp; conditions</license>
  <remotePackage path="build-tools;37.0.0">
    <revision><major>37</major><minor>0</minor><micro>3</micro></revision>
    <archives>
      <archive>
        <complete><size>{size}</size><checksum type="sha1">0000000000000000000000000000000000000000</checksum><url>{name}</url></complete>
        <host-os>windows</host-os>
      </archive>
    </archives>
  </remotePackage>
  <remotePackage path="platform-tools">
    <type-details xsi:type="generic:genericDetailsType"/>
    <revision>
      <major>{major}</major>
      <minor>{minor}</minor>
      <micro>{micro}</micro>
    </revision>
    <display-name>Android SDK Platform-Tools</display-name>
    <archives>
      <archive>
        <complete><size>9571024</size><checksum type="sha1">e1579b288a7ae4e9bc28dedec6a154d78b3c04ed</checksum><url>platform-tools_r{version}-linux.zip</url></complete>
        <host-os>linux</host-os>
      </archive>
      <archive>
        <complete>
          <size>{size}</size>
          <checksum type="sha1">{sha1}</checksum>
          <url>{name}</url>
        </complete>
        <host-os>windows</host-os>
      </archive>
    </archives>
  </remotePackage>
</sdk:sdk-repository>
"#
        )
    }

    /// Google republished the archive: the server sends `received`, the
    /// pin says `pinned`, the index is `index`. Returns the outcome and the
    /// install folder.
    async fn republished(
        pinned: &[u8],
        received: Vec<u8>,
        index: String,
        verify: impl Fn(&Path) -> Result<(), String>,
    ) -> (
        Result<(PathBuf, Option<FromIndex>), PlatformToolsError>,
        tempfile::TempDir,
    ) {
        let base = serve_routes(vec![
            ("/platform-tools_r37.0.3-win.zip", received),
            ("/repository2-3.xml", index.into_bytes()),
        ])
        .await;
        let mut archive = archive(format!("{base}/platform-tools_r37.0.3-win.zip"), pinned);
        archive.index_url = format!("{base}/repository2-3.xml");
        let tmp = tempfile::tempdir().unwrap();
        let result = install(
            &http(),
            &archive,
            &tmp.path().join("platform-tools"),
            verify,
        )
        .await;
        (result, tmp)
    }

    #[tokio::test]
    async fn a_republished_archive_is_accepted_from_googles_index() {
        let pinned = archive_bytes(&[]);
        let new = archive_bytes(&[("platform-tools/NOTICE2.txt", b"republished")]);
        let sha1 = hex(&Sha1::digest(&new));
        let index = index("37.0.3", "platform-tools_r37.0.3-win.zip", new.len(), &sha1);
        let checked = std::sync::Mutex::new(0);
        let (result, tmp) = republished(&pinned, new.clone(), index, |_| {
            *checked.lock().unwrap() += 1;
            Ok(())
        })
        .await;

        let (adb, from_index) = result.unwrap();
        let expected = FromIndex {
            version: "37.0.3".into(),
            size: new.len() as u64,
            sha1,
            sha256: hex(&Sha256::digest(&new)),
            pinned_sha256: hex(&Sha256::digest(&pinned)),
        };
        assert_eq!(from_index.as_ref(), Some(&expected));
        assert_eq!(std::fs::read(&adb).unwrap(), b"adb");
        // The signatures are still checked: adb.exe and AdbWinApi.dll.
        assert_eq!(*checked.lock().unwrap(), 2);
        // Kept next to the tools, for the USB card.
        let dir = tmp.path().join("platform-tools");
        assert_eq!(read_from_index(&dir), Some(expected));
    }

    #[tokio::test]
    async fn the_index_must_list_the_same_archive() {
        let pinned = archive_bytes(&[]);
        let new = archive_bytes(&[("platform-tools/evil.exe", b"x")]);
        let sha1 = hex(&Sha1::digest(&new));
        let name = "platform-tools_r37.0.3-win.zip";
        for index in [
            // Another SHA-1.
            index("37.0.3", name, new.len(), &"ab".repeat(20)),
            // Another size.
            index("37.0.3", name, new.len() + 1, &sha1),
            // Another version (a newer one is never taken).
            index("37.0.4", name, new.len(), &sha1),
            // Another file name.
            index(
                "37.0.3",
                "platform-tools_r37.0.3-windows.zip",
                new.len(),
                &sha1,
            ),
            // Not an index.
            "<html>maintenance</html>".to_owned(),
        ] {
            let (result, tmp) = republished(&pinned, new.clone(), index, |_| Ok(())).await;
            assert!(
                matches!(
                    result,
                    Err(PlatformToolsError::Checksum {
                        reason: Mismatch::Changed,
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn an_archive_from_the_index_still_needs_googles_signatures() {
        let pinned = archive_bytes(&[]);
        let new = archive_bytes(&[("platform-tools/NOTICE2.txt", b"republished")]);
        let sha1 = hex(&Sha1::digest(&new));
        let index = index("37.0.3", "platform-tools_r37.0.3-win.zip", new.len(), &sha1);
        let (result, tmp) = republished(&pinned, new, index, |path| {
            if path.ends_with("adb.exe") {
                Err("signed by \"Someone\"".into())
            } else {
                Ok(())
            }
        })
        .await;
        assert!(
            matches!(result, Err(PlatformToolsError::Signature { .. })),
            "{result:?}"
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn the_index_is_read() {
        let xml = index(
            "37.0.3",
            "platform-tools_r37.0.3-win.zip",
            8_188_601,
            "7fea85bc18d0480bc81d3603952d666f49496e5a",
        );
        assert_eq!(
            windows_archive(&xml, "37.0.3"),
            Ok(Some(IndexArchive {
                url: "platform-tools_r37.0.3-win.zip".into(),
                size: 8_188_601,
                sha1: "7fea85bc18d0480bc81d3603952d666f49496e5a".into(),
                host_os: "windows".into(),
            }))
        );
        assert_eq!(windows_archive(&xml, "37.0.2"), Ok(None));
        assert_eq!(windows_archive("<sdk/>", "37.0.3"), Ok(None));
        assert!(windows_archive("<a><b></a>", "37.0.3").is_err());
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
        let (adb, from_index) = install(&http(), &archive(url, &bytes), &dir, |path| {
            checked
                .lock()
                .unwrap()
                .push(path.file_name().unwrap().to_owned());
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(from_index, None);
        assert_eq!(read_from_index(&dir), None);

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
        assert!(
            matches!(
                result,
                Err(PlatformToolsError::Checksum {
                    reason: Mismatch::Changed,
                    expected,
                    ..
                }) if expected == bytes.len() as u64
            ),
            "{result:?}"
        );
        assert_eq!(std::fs::read(dir.join("adb.exe")).unwrap(), b"old");
    }

    /// Installs from `url` an archive expected to be `bytes`; returns why it
    /// was refused.
    async fn refused(url: String, bytes: &[u8]) -> PlatformToolsError {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("platform-tools");
        let result = install(&http(), &archive(url, bytes), &dir, |_| Ok(())).await;
        assert!(!dir.exists());
        result.unwrap_err()
    }

    /// A Wi-Fi portal or a proxy answering with its own page.
    #[tokio::test]
    async fn a_web_page_is_recognised() {
        let bytes = archive_bytes(&[]);
        let page = b"\n<!DOCTYPE html><html><title>Hotspot login</title></html>".to_vec();
        let url = serve(200, page.clone()).await;
        assert_eq!(
            refused(url, &bytes).await,
            PlatformToolsError::Checksum {
                reason: Mismatch::Html,
                received: page.len() as u64,
                expected: bytes.len() as u64,
            }
        );
        // Recognised by its type too, whatever it starts with.
        let url = serve_with(
            200,
            "content-type: text/html; charset=utf-8\r\n",
            b"x".to_vec(),
            1,
        )
        .await;
        assert!(matches!(
            refused(url, &bytes).await,
            PlatformToolsError::Checksum {
                reason: Mismatch::Html,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn a_cut_download_is_recognised() {
        let bytes = archive_bytes(&[]);
        let half = bytes[..bytes.len() / 2].to_vec();
        // The server stops early (more bytes announced).
        let url = serve_with(200, "", half.clone(), bytes.len()).await;
        assert_eq!(
            refused(url, &bytes).await,
            PlatformToolsError::Checksum {
                reason: Mismatch::Truncated,
                received: half.len() as u64,
                expected: bytes.len() as u64,
            }
        );
        // Or announces and sends only part of the file.
        let url = serve(200, half).await;
        assert!(matches!(
            refused(url, &bytes).await,
            PlatformToolsError::Checksum {
                reason: Mismatch::Truncated,
                ..
            }
        ));
    }

    #[test]
    fn mismatches_are_told_apart() {
        let received = |bytes: &[u8], content_type: Option<&str>| Received {
            bytes: bytes.to_vec(),
            status: reqwest::StatusCode::OK,
            content_type: content_type.map(str::to_owned),
            final_url: "https://example.invalid/a.zip".into(),
            cut: None,
        };
        let zip = archive_bytes(&[]);
        // A whole archive, even smaller than the pinned one: republished.
        assert_eq!(mismatch(&received(&zip, None), 1 << 30), Mismatch::Changed);
        assert_eq!(
            mismatch(&received(&zip, Some("application/zip")), 10),
            Mismatch::Changed
        );
        assert_eq!(
            mismatch(&received(&zip[..zip.len() - 1], None), 1 << 30),
            Mismatch::Truncated
        );
        assert_eq!(
            mismatch(&received(b"\xEF\xBB\xBF  <html>", None), 1),
            Mismatch::Html
        );
        assert_eq!(
            mismatch(&received(b"PK", Some(" TEXT/HTML")), 1),
            Mismatch::Html
        );
        assert_eq!(
            mismatch(&received(&[7; 64], None), 64),
            Mismatch::Unexpected
        );
        let mut over = received(&zip, None);
        over.cut = Some("more than 10 bytes".into());
        assert_eq!(mismatch(&over, 5), Mismatch::Truncated);
    }

    #[test]
    fn the_log_shows_the_first_bytes() {
        assert_eq!(preview(b""), "");
        assert_eq!(preview(b"PK\x03\x04\x14\x00"), "504b03041400");
        assert_eq!(
            preview(b"<html>\n  <title>Portal</title>"),
            "3c68746d6c3e0a20203c7469746c653e \"<html> <title>Portal</title>\""
        );
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
