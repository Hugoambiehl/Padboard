//! The Windows installer of an update: downloaded, checked (its minisign
//! signature for this file and version, and its SHA-256 in the release's
//! signed `SHA256SUMS`), saved where the user finds it, then **shown**, not
//! started, unless Windows says it carries a valid Authenticode signature
//! by [`TRUSTED_PUBLISHER`].
//!
//! Why: an unsigned installer started by Padboard looked like a dropper to
//! Bitdefender's behaviour analysis (ATC, `Atc4.Detection`, rc.1 → rc.2):
//! the installer, Padboard itself and its preferences were quarantined, and
//! the user was left without Padboard and without a message. The same file
//! started by the user (double-click) installs fine. So, until SignPath
//! signs the installer, Padboard opens the Explorer on the checked file and
//! asks the user to quit Padboard and run it.

use std::path::{Path, PathBuf};

use super::UpdateError;
use super::assets;
use super::signing::{CHECKSUMS, CHECKSUMS_SIGNATURE, Checksums, UpdateKey};

/// The Authenticode signer (certificate simple name, as Windows reports
/// it) whose installers Padboard may start by itself. Empty until SignPath
/// signs the releases: then no installer is ever started automatically.
pub const TRUSTED_PUBLISHER: &str = "";

/// An installer is at most this large (the real one is about 10 MB).
const MAX_INSTALLER: usize = 256 * 1024 * 1024;

/// What to do with a checked installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// Signed by [`TRUSTED_PUBLISHER`]: Padboard starts it.
    Start,
    /// Anything else: shown in the Explorer, the user starts it.
    Show { why: String },
}

/// Starts the installer only if Windows validated its Authenticode
/// signature (`signer`: the result of the check, see
/// [`crate::platform_tools::authenticode::signer`]) and it was made by
/// `trusted` (never when `trusted` is empty).
#[must_use]
pub fn decide(signer: &Result<String, String>, trusted: &str) -> Launch {
    let show = |why: String| Launch::Show { why };
    if trusted.is_empty() {
        return show("no trusted publisher in this build".into());
    }
    match signer {
        Ok(name) if name == trusted => Launch::Start,
        Ok(name) => show(format!("signed by {name:?}, not {trusted:?}")),
        Err(e) => show(e.clone()),
    }
}

/// Who validly signed `path` (Authenticode), as Windows says.
///
/// # Errors
/// Not signed, not trusted, or not on Windows.
pub fn signer(path: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        crate::platform_tools::authenticode::signer(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("no Authenticode on this system".into())
    }
}

/// Where the checked installer goes, and how it is downloaded.
#[derive(Debug)]
pub struct Download<'a> {
    pub http: &'a reqwest::Client,
    pub key: &'a UpdateKey,
    /// `…/releases` ([`super::source::Source::releases_url`]).
    pub releases_url: &'a str,
    pub version: &'a str,
    /// The folder the installer is saved in (Downloads; the test dir in
    /// test mode).
    pub dir: &'a Path,
}

impl Download<'_> {
    /// Downloads and checks the installer, then saves it in `dir`: its
    /// path, and the checked bytes. `progress` gets (bytes received, total
    /// if known).
    ///
    /// # Errors
    /// Network, signature, or the file cannot be saved.
    pub async fn run(
        &self,
        mut progress: impl FnMut(u64, Option<u64>),
    ) -> Result<(PathBuf, Vec<u8>), UpdateError> {
        let name = assets::installer(self.version);
        let sums = self.fetch(CHECKSUMS).await?;
        let sums_sig = text(self.fetch(CHECKSUMS_SIGNATURE).await?)?;
        let sig = text(self.fetch(&format!("{name}.sig")).await?)?;
        let installer = self.fetch_with_progress(&name, &mut progress).await?;
        check(
            self.key,
            self.version,
            &name,
            &installer,
            &sig,
            &sums,
            &sums_sig,
        )?;
        let path = self.dir.join(&name);
        save(&path, &installer).map_err(|e| {
            tracing::error!("cannot save the installer in {}: {e}", self.dir.display());
            UpdateError::Storage
        })?;
        tracing::info!(version = self.version, path = %path.display(), "installer downloaded and checked");
        Ok((path, installer))
    }

    async fn fetch(&self, file: &str) -> Result<Vec<u8>, UpdateError> {
        self.fetch_with_progress(file, &mut |_, _| {}).await
    }

    async fn fetch_with_progress(
        &self,
        file: &str,
        progress: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>, UpdateError> {
        let url = assets::download_url(self.releases_url, self.version, file);
        let network = |e: reqwest::Error| {
            tracing::warn!("update: {url}: {e}");
            UpdateError::Network
        };
        let mut response = self.http.get(&url).send().await.map_err(network)?;
        if !response.status().is_success() {
            tracing::warn!("update: {url}: {}", response.status());
            return Err(UpdateError::Network);
        }
        let total = response.content_length();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_INSTALLER {
                tracing::error!("update: {url} is too large");
                return Err(UpdateError::Signature);
            }
            progress(bytes.len() as u64, total);
        }
        Ok(bytes)
    }
}

fn text(bytes: Vec<u8>) -> Result<String, UpdateError> {
    String::from_utf8(bytes).map_err(|_| UpdateError::Signature)
}

/// The installer is the release's: its own signature (for this file name
/// and version) and its line in the signed `SHA256SUMS`.
fn check(
    key: &UpdateKey,
    version: &str,
    name: &str,
    installer: &[u8],
    signature: &str,
    sums: &[u8],
    sums_signature: &str,
) -> Result<(), UpdateError> {
    let refused = |e: super::signing::SigningError| {
        tracing::error!("update refused: {e}");
        UpdateError::Signature
    };
    key.verify(installer, signature, name, version)
        .map_err(refused)?;
    Checksums::verify(key, sums, sums_signature, version)
        .map_err(refused)?
        .check(name, installer)
        .map_err(refused)
}

/// Written whole or not at all (a `.partial` file renamed).
fn save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&partial);
    })
}

/// Opens the Explorer on `path`, the file selected (Windows, test mode
/// included: the manual tests of the update need it). Elsewhere only
/// logged: Linux installs updates by hand (the .deb), only test builds
/// (e2e tests) get here, and they never open windows on the desktop.
///
/// # Errors
/// The Explorer could not be started.
pub fn show(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // Explorer wants `/select,"<path>"` as one argument, quoted inside:
        // Rust's own quoting would wrap the whole of it.
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map(drop)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        tracing::info!(path = %path.display(), "installer saved (not shown outside Windows)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_valid_signature_by_the_trusted_publisher_starts_the_installer() {
        let signed = |name: &str| Ok::<_, String>(name.to_owned());
        let unsigned =
            Err::<String, _>("not validly signed (WinVerifyTrust 0x800B0100)".to_owned());

        // This build: no trusted publisher, so never.
        assert!(TRUSTED_PUBLISHER.is_empty());
        assert!(matches!(
            decide(&signed("Anyone"), TRUSTED_PUBLISHER),
            Launch::Show { .. }
        ));
        assert!(matches!(
            decide(&unsigned, TRUSTED_PUBLISHER),
            Launch::Show { .. }
        ));
        assert!(matches!(decide(&signed(""), ""), Launch::Show { .. }));

        // Once SignPath signs: only its signer.
        let trusted = "SignPath Foundation";
        assert_eq!(decide(&signed(trusted), trusted), Launch::Start);
        assert!(matches!(decide(&unsigned, trusted), Launch::Show { .. }));
        assert!(matches!(
            decide(&signed("Google LLC"), trusted),
            Launch::Show { .. }
        ));
        assert!(matches!(
            decide(&signed("signpath foundation"), trusted),
            Launch::Show { .. }
        ));
    }

    #[test]
    fn no_authenticode_outside_windows() {
        if !cfg!(windows) {
            assert!(signer(Path::new("/bin/sh")).is_err());
        }
    }

    const NAME: &str = "Padboard_1.2.3_x64-setup.exe";
    const KEY: &str = include_str!("fixtures/installer/test-key.pub");
    const INSTALLER: &[u8] = include_bytes!("fixtures/installer/Padboard_1.2.3_x64-setup.exe");
    const INSTALLER_SIG: &[u8] =
        include_bytes!("fixtures/installer/Padboard_1.2.3_x64-setup.exe.sig");
    const SUMS: &[u8] = include_bytes!("fixtures/installer/SHA256SUMS");
    const SUMS_SIG: &[u8] = include_bytes!("fixtures/installer/SHA256SUMS.sig");

    /// A release server for `/releases/download/v1.2.3/<file>` (404 else).
    async fn server(files: Vec<(&'static str, Vec<u8>)>) -> String {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/releases", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = vec![0u8; 4096];
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..n]).into_owned();
                let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
                let body = files
                    .iter()
                    .find(|(name, _)| path == format!("/releases/download/v1.2.3/{name}"))
                    .map(|(_, body)| body.clone());
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
                if let Some(b) = body {
                    let _ = socket.write_all(&b).await;
                }
            }
        });
        base
    }

    fn release(installer: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (NAME, installer.to_vec()),
            ("Padboard_1.2.3_x64-setup.exe.sig", INSTALLER_SIG.to_vec()),
            (CHECKSUMS, SUMS.to_vec()),
            (CHECKSUMS_SIGNATURE, SUMS_SIG.to_vec()),
        ]
    }

    async fn download(base: &str, version: &str, dir: &Path) -> Result<PathBuf, UpdateError> {
        let http = super::super::client().unwrap();
        let key = UpdateKey::from_tauri(KEY).unwrap();
        let mut seen = 0;
        let result = Download {
            http: &http,
            key: &key,
            releases_url: base,
            version,
            dir,
        }
        .run(|received, _| seen = received)
        .await
        .map(|(path, bytes)| {
            assert_eq!(bytes, INSTALLER);
            path
        });
        if result.is_ok() {
            assert_eq!(seen, INSTALLER.len() as u64, "progress reported");
        }
        result
    }

    #[tokio::test]
    async fn the_checked_installer_is_saved() {
        let base = server(release(INSTALLER)).await;
        let tmp = tempfile::tempdir().unwrap();
        let path = download(&base, "1.2.3", tmp.path()).await.unwrap();
        assert_eq!(path, tmp.path().join(NAME));
        assert_eq!(std::fs::read(&path).unwrap(), INSTALLER);
    }

    #[tokio::test]
    async fn a_tampered_installer_is_refused_and_not_saved() {
        let base = server(release(b"not a real installer, version 6.6.6\n")).await;
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            download(&base, "1.2.3", tmp.path()).await,
            Err(UpdateError::Signature)
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn missing_files_are_a_network_error() {
        let mut files = release(INSTALLER);
        files.retain(|(name, _)| *name != CHECKSUMS_SIGNATURE);
        let base = server(files).await;
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            download(&base, "1.2.3", tmp.path()).await,
            Err(UpdateError::Network)
        );
    }

    #[test]
    fn the_fixtures_check_only_for_their_version() {
        let key = UpdateKey::from_tauri(KEY).unwrap();
        let sig = std::str::from_utf8(INSTALLER_SIG).unwrap();
        let sums_sig = std::str::from_utf8(SUMS_SIG).unwrap();
        assert_eq!(
            check(&key, "1.2.3", NAME, INSTALLER, sig, SUMS, sums_sig),
            Ok(())
        );
        assert_eq!(
            check(&key, "1.2.4", NAME, INSTALLER, sig, SUMS, sums_sig),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn saving_replaces_the_file_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("Padboard_1.2.3_x64-setup.exe");
        save(&path, b"first").unwrap();
        save(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }
}
