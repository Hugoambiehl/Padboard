//! What a release contains and where it is downloaded from (GitHub
//! releases of the repository). `scripts/sign-release.sh` and the release
//! workflow produce these names.

/// The repository, as GitHub spells it.
pub const REPOSITORY: &str = "Hugoambiehl/Padboard";

/// Pages and downloads of releases.
pub const RELEASES_URL: &str = "https://github.com/Hugoambiehl/Padboard/releases";
/// The releases API (the list, including pre-releases).
pub const RELEASES_API: &str = "https://api.github.com/repos/Hugoambiehl/Padboard/releases";

/// The update manifest read by `tauri-plugin-updater`.
pub const MANIFEST: &str = "latest.json";

/// The tag of `version`.
#[must_use]
pub fn tag(version: &str) -> String {
    format!("v{version}")
}

/// The NSIS installer of `version` (named by the Tauri bundler).
#[must_use]
pub fn installer(version: &str) -> String {
    format!("Padboard_{version}_x64-setup.exe")
}

/// The signed tablet APK of `version`.
#[must_use]
pub fn apk(version: &str) -> String {
    format!("padboard-{version}.apk")
}

/// Where `file` of `version` is downloaded from, under `base` (normally
/// [`RELEASES_URL`]).
#[must_use]
pub fn download_url(base: &str, version: &str, file: &str) -> String {
    format!(
        "{}/download/{}/{file}",
        base.trim_end_matches('/'),
        tag(version)
    )
}

/// The release page of `version`.
#[must_use]
pub fn page_url(version: &str) -> String {
    format!("{RELEASES_URL}/tag/{}", tag(version))
}

/// The tablet's `versionCode` for `version` (as `scripts/release.sh` writes
/// it): MAJOR·10⁶ + MINOR·10⁴ + PATCH·100 + the release candidate number,
/// 99 for the release itself. `None` for a version outside this scheme.
#[must_use]
pub fn version_code(version: &str) -> Option<u32> {
    let parsed = semver::Version::parse(version).ok()?;
    if !parsed.build.is_empty() {
        return None;
    }
    let rc = match parsed.pre.as_str() {
        "" => 99,
        pre => pre
            .strip_prefix("rc.")?
            .parse::<u32>()
            .ok()
            .filter(|n| (1..99).contains(n))?,
    };
    let (major, minor, patch) = (
        u32::try_from(parsed.major).ok()?,
        u32::try_from(parsed.minor).ok()?,
        u32::try_from(parsed.patch).ok()?,
    );
    if minor > 99 || patch > 99 {
        return None;
    }
    major
        .checked_mul(1_000_000)?
        .checked_add(minor * 10_000 + patch * 100 + rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_urls() {
        assert_eq!(installer("0.2.0"), "Padboard_0.2.0_x64-setup.exe");
        assert_eq!(apk("0.2.0-rc.1"), "padboard-0.2.0-rc.1.apk");
        assert_eq!(
            download_url(RELEASES_URL, "0.2.0", "SHA256SUMS"),
            "https://github.com/Hugoambiehl/Padboard/releases/download/v0.2.0/SHA256SUMS"
        );
        assert_eq!(
            page_url("0.2.0"),
            "https://github.com/Hugoambiehl/Padboard/releases/tag/v0.2.0"
        );
    }

    #[test]
    fn version_codes_follow_the_release_script() {
        assert_eq!(version_code("0.1.0"), Some(10_099));
        assert_eq!(version_code("0.1.0-rc.4"), Some(10_004));
        assert_eq!(version_code("0.2.0-rc.1"), Some(20_001));
        assert_eq!(version_code("1.2.3"), Some(1_020_399));
        assert!(version_code("0.2.0-rc.1").unwrap() < version_code("0.2.0").unwrap());
        assert_eq!(version_code("0.2.0-beta.1"), None);
        assert_eq!(version_code("0.100.0"), None);
        assert_eq!(version_code("nonsense"), None);
    }
}
