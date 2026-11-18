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
//!
//! With `VIRUSTOTAL_API_KEY` set (a free VirusTotal account's public API key,
//! never committed or printed), it then shows VirusTotal's latest analysis of
//! the installer, looked up by its SHA-256 (the file is never uploaded): the
//! number of detections and the engines, and a warning when one of the
//! limits of `docs/releasing.md` is passed. Informative only: it never
//! changes the exit status (the file may not be on VirusTotal yet).

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

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
        Ok(installer_sha256) => {
            println!("release {version}: signatures, checksums and latest.json are consistent");
            match std::env::var(VIRUSTOTAL_KEY_VARIABLE) {
                Ok(api_key) if !api_key.trim().is_empty() => {
                    virustotal(&installer_sha256, api_key.trim());
                }
                _ => println!("VirusTotal: not checked ({VIRUSTOTAL_KEY_VARIABLE} not set)"),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Checks the folder; returns the installer's SHA-256 (hex).
fn verify(dir: &Path, version: &str, key: Option<&Path>) -> Result<String, String> {
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
    let sha256 = sums
        .get(&installer)
        .ok_or_else(|| format!("{installer} is not in {CHECKSUMS}"))?;
    Ok(sha256.iter().map(|b| format!("{b:02x}")).collect())
}

const VIRUSTOTAL_KEY_VARIABLE: &str = "VIRUSTOTAL_API_KEY";
/// More generic verdicts than this: stop (`docs/releasing.md`).
const MAX_DETECTIONS: usize = 3;
/// Engines that must never detect the installer.
const WATCHED_ENGINES: [&str; 2] = ["BitDefender", "Microsoft"];

/// Prints VirusTotal's latest analysis of the file `sha256`.
fn virustotal(sha256: &str, api_key: &str) {
    let link = format!("https://www.virustotal.com/gui/file/{sha256}");
    match fetch_report(sha256, api_key) {
        Ok(Some(body)) => match Report::parse(&body) {
            Ok(report) => {
                for line in report.lines() {
                    println!("{line}");
                }
                println!("VirusTotal: {link}");
            }
            Err(e) => eprintln!("VirusTotal: unreadable answer ({e})"),
        },
        Ok(None) => println!("VirusTotal: the installer is not there yet; upload it ({link})"),
        Err(e) => eprintln!("VirusTotal: not checked ({e})"),
    }
}

/// The file report (`None`: unknown to VirusTotal).
fn fetch_report(sha256: &str, api_key: &str) -> Result<Option<Vec<u8>>, String> {
    // reqwest is built without a crypto provider (see updates::client).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let http = reqwest::Client::builder()
            .user_agent("Padboard verify-release")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let response = http
            .get(format!("https://www.virustotal.com/api/v3/files/{sha256}"))
            .header("x-apikey", api_key)
            .send()
            .await
            // Never the request itself: it carries the key.
            .map_err(|e| format!("request failed: {}", e.without_url()))?;
        match response.status().as_u16() {
            200 => {}
            404 => return Ok(None),
            401 | 403 => return Err(format!("{VIRUSTOTAL_KEY_VARIABLE} refused")),
            429 => return Err("quota of the public API reached, try again later".to_owned()),
            status => return Err(format!("VirusTotal answered {status}")),
        }
        let body = response
            .bytes()
            .await
            .map_err(|e| e.without_url().to_string())?;
        Ok(Some(body.to_vec()))
    })
}

/// What a VirusTotal file report says.
#[derive(Debug, PartialEq)]
struct Report {
    /// Engines with a verdict (detected or not).
    engines: usize,
    /// (engine, category, result), sorted by engine.
    detections: Vec<(String, String, String)>,
    /// Unix time of the analysis.
    date: Option<i64>,
}

impl Report {
    fn parse(body: &[u8]) -> Result<Self, String> {
        let json: serde_json::Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
        let attributes = &json["data"]["attributes"];
        let results = attributes["last_analysis_results"]
            .as_object()
            .ok_or("no last_analysis_results")?;
        let mut engines = 0;
        let mut detections = Vec::new();
        for (engine, result) in results {
            let category = result["category"].as_str().unwrap_or_default();
            match category {
                "malicious" | "suspicious" => {
                    engines += 1;
                    let verdict = result["result"].as_str().unwrap_or(category);
                    detections.push((engine.clone(), category.to_owned(), verdict.to_owned()));
                }
                "undetected" | "harmless" => engines += 1,
                // type-unsupported, timeout, failure: no verdict.
                _ => {}
            }
        }
        detections.sort();
        Ok(Self {
            engines,
            detections,
            date: attributes["last_analysis_date"].as_i64(),
        })
    }

    /// The analysis, for the terminal.
    fn lines(&self) -> Vec<String> {
        let age = self.date.map_or_else(String::new, |date| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs().cast_signed());
            format!(" (analysed {} h ago)", (now - date).max(0) / 3600)
        });
        let mut lines = vec![format!(
            "VirusTotal: {}/{}{age}",
            self.detections.len(),
            self.engines
        )];
        for (engine, category, verdict) in &self.detections {
            lines.push(format!("  {engine}: {verdict} ({category})"));
        }
        for watched in WATCHED_ENGINES {
            if self
                .detections
                .iter()
                .any(|(engine, ..)| engine.eq_ignore_ascii_case(watched))
            {
                lines.push(format!("STOP: {watched} detects it (docs/releasing.md)"));
            }
        }
        if self.detections.len() > MAX_DETECTIONS {
            lines.push(format!(
                "STOP: more than {MAX_DETECTIONS} detections (docs/releasing.md)"
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(results: &serde_json::Value) -> Report {
        let body = serde_json::json!({ "data": { "attributes": {
            "last_analysis_date": 1_700_000_000,
            "last_analysis_results": results,
        }}});
        Report::parse(body.to_string().as_bytes()).unwrap()
    }

    #[test]
    fn counts_engines_with_a_verdict() {
        let report = report(&serde_json::json!({
            "Sophos": { "category": "malicious", "result": "Generic ML PUA (PUA)" },
            "SecureAge": { "category": "malicious", "result": "Malicious" },
            "Acme": { "category": "suspicious", "result": null },
            "BitDefender": { "category": "undetected", "result": null },
            "Other": { "category": "harmless", "result": null },
            "Mac": { "category": "type-unsupported", "result": null },
            "Slow": { "category": "timeout", "result": null },
        }));
        assert_eq!(report.engines, 5);
        assert_eq!(
            report.detections,
            [
                ("Acme".into(), "suspicious".into(), "suspicious".into()),
                ("SecureAge".into(), "malicious".into(), "Malicious".into()),
                (
                    "Sophos".into(),
                    "malicious".into(),
                    "Generic ML PUA (PUA)".into()
                ),
            ]
        );
        let lines = report.lines();
        assert!(lines[0].starts_with("VirusTotal: 3/5 (analysed "));
        assert_eq!(lines[3], "  Sophos: Generic ML PUA (PUA) (malicious)");
        assert!(!lines.iter().any(|l| l.starts_with("STOP")));
    }

    #[test]
    fn warns_on_watched_engines_and_too_many_detections() {
        let detected = serde_json::json!({ "category": "malicious", "result": "X" });
        let report = report(&serde_json::json!({
            "BitDefender": detected, "Microsoft": detected,
            "A": detected, "B": detected,
        }));
        let stops: Vec<_> = report
            .lines()
            .into_iter()
            .filter(|l| l.starts_with("STOP"))
            .collect();
        assert_eq!(
            stops,
            [
                "STOP: BitDefender detects it (docs/releasing.md)",
                "STOP: Microsoft detects it (docs/releasing.md)",
                "STOP: more than 3 detections (docs/releasing.md)",
            ]
        );
    }

    #[test]
    fn a_report_without_results_is_refused() {
        assert!(Report::parse(br#"{"error":{"code":"NotFoundError"}}"#).is_err());
    }
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
