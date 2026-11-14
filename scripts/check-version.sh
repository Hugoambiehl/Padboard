#!/usr/bin/env bash
# CI check (docs/releasing.md): every file carries the same version, with
# the right Android versionCode, and it is a development version
# (X.Y.Z-dev) except on a release tag (vX.Y.Z or vX.Y.Z-rc.N). A build of
# main can then never be taken for a published version.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/set-version.sh
source "$ROOT/scripts/lib/set-version.sh"

version="$(node -p 'require("./apps/desktop/src-tauri/tauri.conf.json").version')"
parse_version "$version"
fail=0
check() { # what found
  if [[ "$2" != "$version" ]]; then
    echo "error: $1 is $2, tauri.conf.json is $version" >&2
    fail=1
  fi
}
check "Cargo.toml" "$(sed -n '/^\[workspace\.package\]/,/^\[/ s/^version = "\(.*\)"/\1/p' Cargo.toml)"
check "package.json" "$(node -p 'require("./apps/desktop/package.json").version')"
check "package-lock.json" "$(node -p 'require("./apps/desktop/package-lock.json").version')"
check "build.gradle.kts versionName" "$(sed -n 's/^\s*versionName = "\(.*\)"/\1/p' apps/android/app/build.gradle.kts)"
code="$(sed -n 's/^\s*versionCode = \([0-9]*\)$/\1/p' apps/android/app/build.gradle.kts)"
if [[ "$code" != "$version_code" ]]; then
  echo "error: versionCode is $code, $version needs $version_code" >&2
  fail=1
fi
if [[ "$version" != *-dev ]]; then
  if [[ "${GITHUB_REF_TYPE:-}" == tag && "${GITHUB_REF_NAME:-}" == "v$version" ]]; then
    echo "Release tag v$version."
  else
    echo "error: main carries the released version $version; it must be a development version (scripts/dev-version.sh X.Y.Z-dev, release.sh does it after each release)" >&2
    fail=1
  fi
fi
((fail == 0)) && echo "Version $version (versionCode $version_code): consistent."
exit "$fail"
