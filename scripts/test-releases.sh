#!/usr/bin/env bash
# Local releases to test updates without publishing anything (tests only).
# A Padboard built with the `test-updates` feature reads them when
# PADBOARD_TEST_RELEASES points here (apps/desktop/src-tauri/src/updates/source.rs).
#
#   scripts/test-releases.sh add 0.2.0-rc.2 --apk app.apk [--installer setup.exe]
#   scripts/test-releases.sh serve            # http://127.0.0.1:8765 (PORT=…)
#   scripts/test-releases.sh env              # the variables for the test app
#   scripts/test-releases.sh remove 0.2.0-rc.2
#
# Files go to $TEST_RELEASES (default target/test-releases). They are signed
# with a TEST key created there on first use (no password): never the
# release keys of ~/.padboard-signing. Same layout and signatures as a real
# release (SHA256SUMS signed for the version, installer .sig, latest.json).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${TEST_RELEASES:-$root/target/test-releases}"
port="${PORT:-8765}"
base="http://127.0.0.1:$port"
key="$dir/test-update.key"

die() { echo "error: $*" >&2; exit 1; }
tauri() { (cd "$root/apps/desktop" && npx --no-install tauri "$@"); }

ensure_key() {
  mkdir -p "$dir/api" "$dir/releases/download"
  if [[ ! -f "$key" ]]; then
    tauri signer generate --ci -p "" -w "$key" >/dev/null
    echo "Test key created: $key.pub"
  fi
}

sign() { # file version
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" tauri signer sign --app-version "$2" -f "$key" "$1" >/dev/null
}

# The releases list, as GitHub's API gives it (newest first).
write_list() {
  python3 -I - "$dir" <<'EOF'
import json, os, sys
root = sys.argv[1]
downloads = os.path.join(root, "releases", "download")
releases = []
for tag in sorted(os.listdir(downloads), reverse=True):
    files = sorted(os.listdir(os.path.join(downloads, tag)))
    releases.append({
        "tag_name": tag,
        "draft": False,
        "prerelease": "-rc." in tag,
        "assets": [{"name": f} for f in files],
    })
with open(os.path.join(root, "api", "releases"), "w") as f:
    json.dump(releases, f, indent=2)
EOF
}

case "${1:-}" in
  add)
    version="${2:?version}"
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]] || die "not a release version: $version"
    shift 2
    apk="" installer=""
    while (($#)); do
      case "$1" in
        --apk) apk="${2:?}"; shift 2 ;;
        --installer) installer="${2:?}"; shift 2 ;;
        *) die "unknown option $1" ;;
      esac
    done
    ensure_key
    out="$dir/releases/download/v$version"
    rm -rf -- "$out"
    mkdir -p "$out"
    if [[ -n "$apk" ]]; then
      cp -- "$apk" "$out/padboard-$version.apk"
    fi
    if [[ -n "$installer" ]]; then
      name="Padboard_${version}_x64-setup.exe"
      cp -- "$installer" "$out/$name"
      sign "$out/$name" "$version"
      python3 -I - "$out" "$version" "$name" "$base" <<'EOF'
import json, os, sys
out, version, name, base = sys.argv[1:]
with open(os.path.join(out, name + ".sig")) as f:
    entry = {"signature": f.read().strip(), "url": f"{base}/releases/download/v{version}/{name}"}
with open(os.path.join(out, "latest.json"), "w") as f:
    json.dump({"version": version, "notes": "test", "pub_date": "2026-01-01T00:00:00Z",
               "platforms": {"windows-x86_64": entry, "windows-x86_64-nsis": entry}}, f, indent=2)
EOF
    fi
    (cd "$out" && find . -maxdepth 1 -type f ! -name 'SHA256SUMS*' -printf '%f\n' | sort \
      | xargs -r -d '\n' sha256sum -- > SHA256SUMS)
    sign "$out/SHA256SUMS" "$version"
    write_list
    echo "Test release $version in $out"
    ;;
  remove)
    rm -rf -- "$dir/releases/download/v${2:?version}"
    write_list
    ;;
  serve)
    [[ -d "$dir" ]] || die "no test release yet (add one)"
    echo "Serving $dir on $base (Ctrl+C to stop)"
    exec python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$dir"
    ;;
  env)
    echo "PADBOARD_TEST_RELEASES=$base"
    echo "PADBOARD_TEST_UPDATE_KEY=$key.pub"
    echo "PADBOARD_TEST_APP_VERSION=<the version the test app pretends to be>"
    ;;
  *)
    sed -n '2,15p' "$0"
    exit 2
    ;;
esac
