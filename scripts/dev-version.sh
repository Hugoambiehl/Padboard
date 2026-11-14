#!/usr/bin/env bash
# Sets main's development version by hand (release.sh does it after each
# release): scripts/dev-version.sh 0.2.0-dev. Commit the result.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/set-version.sh
source "$ROOT/scripts/lib/set-version.sh"
version="${1:-}"
[[ "$version" == *-dev ]] || { echo "usage: $0 MAJOR.MINOR.PATCH-dev" >&2; exit 2; }
parse_version "$version"
write_version "$version"
echo "Version $version (Android versionCode $version_code) written: ${VERSION_FILES[*]}"
