//! Which release is the newest: GitHub's list of releases (drafts are not
//! listed to anonymous callers), pre-releases only when "Versions de test"
//! is on.

use semver::Version;
use serde::Deserialize;

/// What the releases API lists (only the fields used).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<ApiAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiAsset {
    pub name: String,
}

/// A release newer than the running app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Newer {
    pub version: Version,
    /// Its `latest.json` is published: the updater can install it.
    pub has_manifest: bool,
}

/// The newest release above `current`, if any. Releases whose tag is not
/// `v<semver>` are ignored; a pre-release (flagged on GitHub or by its
/// version) only counts with `prereleases`.
#[must_use]
pub fn newest(releases: &[ApiRelease], current: &Version, prereleases: bool) -> Option<Newer> {
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.strip_prefix('v')?).ok()?;
            let pre = r.prerelease || !version.pre.is_empty();
            (prereleases || !pre).then(|| Newer {
                has_manifest: r.assets.iter().any(|a| a.name == super::assets::MANIFEST),
                version,
            })
        })
        .filter(|n| n.version > *current)
        .max_by(|a, b| a.version.cmp(&b.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, manifest: bool) -> ApiRelease {
        ApiRelease {
            tag_name: tag.into(),
            draft: false,
            prerelease,
            assets: if manifest {
                vec![ApiAsset {
                    name: "latest.json".into(),
                }]
            } else {
                Vec::new()
            },
        }
    }

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn the_newest_stable_release_wins() {
        let list = [
            release("v0.2.0", false, true),
            release("v0.3.0-rc.1", true, true),
            release("v0.1.0", false, false),
        ];
        assert_eq!(
            newest(&list, &v("0.1.0"), false),
            Some(Newer {
                version: v("0.2.0"),
                has_manifest: true
            })
        );
        assert_eq!(newest(&list, &v("0.3.0-rc.1"), true), None);
    }

    #[test]
    fn prereleases_only_on_the_test_channel() {
        let list = [
            release("v0.2.0", false, true),
            release("v0.3.0-rc.1", true, true),
        ];
        assert_eq!(newest(&list, &v("0.2.0"), false), None);
        assert_eq!(
            newest(&list, &v("0.2.0"), true).map(|n| n.version),
            Some(v("0.3.0-rc.1"))
        );
        // A pre-release version not flagged on GitHub is still one.
        let unflagged = [release("v0.3.0-rc.2", false, true)];
        assert_eq!(newest(&unflagged, &v("0.2.0"), false), None);
    }

    #[test]
    fn a_release_candidate_moves_to_its_release() {
        let list = [release("v0.2.0", false, true)];
        assert_eq!(
            newest(&list, &v("0.2.0-rc.3"), false).map(|n| n.version),
            Some(v("0.2.0"))
        );
    }

    #[test]
    fn never_older_drafts_or_odd_tags() {
        let mut draft = release("v9.0.0", false, true);
        draft.draft = true;
        let list = [
            draft,
            release("nightly", false, true),
            release("v0.0.9", false, true),
        ];
        assert_eq!(newest(&list, &v("0.1.0"), true), None);
    }

    #[test]
    fn a_release_without_manifest_is_still_reported() {
        let list = [release("v0.2.0", false, false)];
        assert_eq!(
            newest(&list, &v("0.1.0"), false),
            Some(Newer {
                version: v("0.2.0"),
                has_manifest: false
            })
        );
    }
}
