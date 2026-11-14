#!/usr/bin/env bash
# Prepares a release: sets the version everywhere, regenerates CHANGELOG.md
# (git-cliff), commits "chore(release): vX" and creates the annotated tag.
# Nothing is pushed: pushing the tag starts .github/workflows/release.yml
# (installers, .deb, unsigned APK, draft GitHub release).
#
#   scripts/release.sh 0.1.0-rc.1
#   scripts/release.sh 0.1.0
#   git push origin main v0.1.0      # then: docs/releasing.md
#
# Versions: MAJOR.MINOR.PATCH, or MAJOR.MINOR.PATCH-rc.N for a release
# candidate (versionCode: scripts/lib/set-version.sh). Right after the
# tagged release commit, a second commit moves main to the next
# development version (X.Y.Z-dev): `git push origin main vX` pushes both,
# the release workflow builds the tag.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# shellcheck source=lib/set-version.sh
source "$ROOT/scripts/lib/set-version.sh"

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]]; then
  echo "usage: $0 MAJOR.MINOR.PATCH[-rc.N]" >&2
  exit 2
fi
parse_version "$version" || exit 2
tag="v$version"

command -v git-cliff >/dev/null || {
  echo "error: git-cliff not found: cargo install git-cliff --locked" >&2
  exit 1
}
[[ "$(git branch --show-current)" == main ]] || { echo "error: release from main" >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "error: the working tree is not clean" >&2; exit 1; }
git fetch -q origin main
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || {
  echo "error: main is not up to date with origin/main" >&2
  exit 1
}
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  echo "error: tag $tag already exists" >&2
  exit 1
fi

write_version "$version"

git-cliff --config cliff.toml --tag "$tag" --output CHANGELOG.md

git add "${VERSION_FILES[@]}" CHANGELOG.md
git commit -q -m "chore(release): $tag"
git tag -a "$tag" -m "Padboard $version"
release_code="$version_code"

# main carries a development version again at once, so a build of main is
# never taken for the release (CI checks it, docs/releasing.md).
dev="$(next_dev_version "$version")"
parse_version "$dev"
write_version "$dev"
git add "${VERSION_FILES[@]}"
git commit -q -m "chore: start $dev"

echo "Release $tag prepared (Android versionCode $release_code), then main moved to $dev."
echo "Check the commit and CHANGELOG.md, then start the release workflow with:"
echo "  git push origin main $tag"
