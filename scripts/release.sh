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
# candidate. Android's versionCode must always grow:
# MAJOR*1000000 + MINOR*10000 + PATCH*100 + (N for rc.N, 99 for the release).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

version="${1:-}"
if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-rc\.([0-9]+))?$ ]]; then
  echo "usage: $0 MAJOR.MINOR.PATCH[-rc.N]" >&2
  exit 2
fi
major="${BASH_REMATCH[1]}" minor="${BASH_REMATCH[2]}" patch="${BASH_REMATCH[3]}" rc="${BASH_REMATCH[5]:-}"
if [[ -n "$rc" ]] && ((rc < 1 || rc > 98)); then
  echo "error: release candidates go from rc.1 to rc.98" >&2
  exit 2
fi
if ((minor > 99 || patch > 99)); then
  echo "error: minor and patch must stay below 100 (Android versionCode)" >&2
  exit 2
fi
version_code=$((major * 1000000 + minor * 10000 + patch * 100 + ${rc:-99}))
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

# Workspace crates (Cargo.toml [workspace.package]) and Cargo.lock.
sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = ".*"/version = "'"$version"'"/' Cargo.toml
cargo update -q --workspace
# Desktop app: Tauri (installer and window) and the frontend package.
node -e '
  const fs = require("fs");
  const [version] = process.argv.slice(1);
  for (const f of ["apps/desktop/src-tauri/tauri.conf.json", "apps/desktop/package.json"]) {
    const json = JSON.parse(fs.readFileSync(f, "utf8"));
    json.version = version;
    fs.writeFileSync(f, JSON.stringify(json, null, 2) + "\n");
  }
  const lockFile = "apps/desktop/package-lock.json";
  const lock = JSON.parse(fs.readFileSync(lockFile, "utf8"));
  lock.version = version;
  lock.packages[""].version = version;
  fs.writeFileSync(lockFile, JSON.stringify(lock, null, 2) + "\n");
' "$version"
# Tablet app.
sed -i -E \
  -e 's/^(\s*versionCode = ).*/\1'"$version_code"'/' \
  -e 's/^(\s*versionName = ).*/\1"'"$version"'"/' \
  apps/android/app/build.gradle.kts

git-cliff --config cliff.toml --tag "$tag" --output CHANGELOG.md

git add Cargo.toml Cargo.lock CHANGELOG.md \
  apps/desktop/src-tauri/tauri.conf.json apps/desktop/package.json apps/desktop/package-lock.json \
  apps/android/app/build.gradle.kts
git commit -q -m "chore(release): $tag"
git tag -a "$tag" -m "Padboard $version"

echo "Release $tag prepared (Android versionCode $version_code)."
echo "Check the commit and CHANGELOG.md, then start the release workflow with:"
echo "  git push origin main $tag"
