//! Checks a release folder before it is uploaded (`scripts/sign-release.sh`),
//! with the same code and key as the app:
//!
//! ```sh
//! cargo run -p padboard-desktop --example verify-release -- <dir> <version> [--key <file.pub>]
//! ```
//!
//! - `SHA256SUMS.sig` is a signature of `SHA256SUMS` for this version, and
//!   every other file of the folder is listed there with the right hash;
//! - the installer's `.sig` is a signature of the installer for this version;
//! - `latest.json` announces this version, the installer's download URL and
//!   that signature;
//! - the signed APK is there, the unsigned one is not.
//!
//! `--key` checks with another public key (tests); by default, the key
//! embedded in the app.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use padboard_desktop_lib::updates::assets;
use padboard_desktop_lib::updates::signing::{
    CHECKSUMS, CHECKSUMS_SIGNATURE, Checksums, UpdateKey,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, version, key) = match args.as_slice() {
        [dir, version] => (PathBuf::from(dir), version.clone(), None),
        [dir, version, flag, key] if flag == "--key" => (
            PathBuf::from(dir),
            version.clone(),
            Some(PathBuf::from(key)),
        ),
        _ => {
            eprintln!("usage: verify-release <dir> <version> [--key <file.pub>]");
            return ExitCode::from(2);
        }
    };
    match verify(&dir, &version, key.as_deref()) {
        Ok(()) => {
            println!("release {version}: signatures, checksums and latest.json are consistent");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn verify(dir: &Path, version: &str, key: Option<&Path>) -> Result<(), String> {
    let key = match key {
        Some(path) => UpdateKey::from_tauri(&read_text(path)?).map_err(|e| e.to_string())?,
        None => UpdateKey::embedded().ok_or("this build has no update key (updater-key.pub)")?,
    };
    if assets::version_code(version).is_none() {
        return Err(format!("{version} is not a release version"));
    }

    let sums = read(&dir.join(CHECKSUMS))?;
    let sums_signature = read_text(&dir.join(CHECKSUMS_SIGNATURE))?;
    let sums = Checksums::verify(&key, &sums, &sums_signature, version)
        .map_err(|e| format!("{CHECKSUMS}: {e}"))?;

    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == CHECKSUMS || name == CHECKSUMS_SIGNATURE {
            continue;
        }
        if name.ends_with("-unsigned.apk") {
            return Err(format!("{name}: the unsigned APK must not be published"));
        }
        sums.check(&name, &read(&entry.path())?)
            .map_err(|e| e.to_string())?;
        names.push(name);
    }

    let apk = assets::apk(version);
    if !names.contains(&apk) {
        return Err(format!("{apk} is missing"));
    }

    let installer = assets::installer(version);
    let installer_signature = read_text(&dir.join(format!("{installer}.sig")))?;
    key.verify(
        &read(&dir.join(&installer))?,
        &installer_signature,
        &installer,
        version,
    )
    .map_err(|e| format!("{installer}: {e}"))?;

    let manifest: serde_json::Value = serde_json::from_slice(&read(&dir.join(assets::MANIFEST))?)
        .map_err(|e| format!("{}: {e}", assets::MANIFEST))?;
    if manifest["version"] != version {
        return Err(format!(
            "{} announces {}",
            assets::MANIFEST,
            manifest["version"]
        ));
    }
    let url = assets::download_url(assets::RELEASES_URL, version, &installer);
    for platform in ["windows-x86_64", "windows-x86_64-nsis"] {
        let entry = &manifest["platforms"][platform];
        if entry["url"] != url.as_str() {
            return Err(format!("{platform}: url {} instead of {url}", entry["url"]));
        }
        if entry["signature"] != installer_signature.trim() {
            return Err(format!("{platform}: not the installer's signature"));
        }
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
