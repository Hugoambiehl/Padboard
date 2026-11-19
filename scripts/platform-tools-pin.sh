#!/usr/bin/env bash
# Pins Google's Platform-Tools for Windows ("Installer adb") to the newest
# version of Google's repository index: version, URL, SHA-256 and size in
# apps/desktop/src-tauri/src/platform_tools/mod.rs. Google only publishes a
# SHA-1: the archive is downloaded, its SHA-1 checked against the index,
# then its SHA-256 computed and written.
#
#   scripts/platform-tools-pin.sh          # print the newest and update the file
#   scripts/platform-tools-pin.sh --check  # only say whether the pin is still right
#
# --check fails when Google's newest version is another one, or when the
# archive changed under the same version (Google republished 37.0.3 on
# 2026-11-19: every download was refused): the index's size and SHA-1 are
# compared with the pin, then the archive is downloaded and its SHA-256
# compared too. Run weekly by .github/workflows/platform-tools.yml.
#
# Then check the new archive's signatures on Windows (docs/windows-test-plan.md,
# "Installer adb") before releasing.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
file="$root/apps/desktop/src-tauri/src/platform_tools/mod.rs"
index_url="https://dl.google.com/android/repository/repository2-3.xml"
base="https://dl.google.com/android/repository"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

curl -sSfL -o "$work/index.xml" "$index_url"
read -r version archive sha1 size < <(python3 -I - "$work/index.xml" <<'PY'
import re, sys
xml = open(sys.argv[1], encoding="utf-8").read()
start = xml.index('path="platform-tools"')
block = xml[start:xml.index("</remotePackage>", start)]
rev = re.search(r"<revision>\s*<major>(\d+)</major>\s*<minor>(\d+)</minor>\s*<micro>(\d+)</micro>", block)
for archive in re.findall(r"<archive>.*?</archive>", block, re.S):
    if "<host-os>windows</host-os>" in archive:
        size = re.search(r"<size>(\d+)</size>", archive).group(1)
        sha1 = re.search(r'<checksum type="sha1">([0-9a-f]{40})</checksum>', archive).group(1)
        url = re.search(r"<url>([^<]+)</url>", archive).group(1)
        print(".".join(rev.groups()), url, sha1, size)
        break
PY
)
pinned() { sed -n "s/^pub const $1: [^=]* = \"\\{0,1\\}\\([^\";]*\\)\"\\{0,1\\};/\\1/p" "$file"; }
current="$(pinned VERSION)"
echo "Pinned: $current; newest from Google: $version ($archive, $size bytes, SHA-1 $sha1)"
[[ "$archive" =~ ^platform-tools_r[0-9.]+-win\.zip$ ]] || { echo "unexpected archive name: $archive" >&2; exit 1; }

curl -sSfL -o "$work/archive.zip" "$base/$archive"
echo "$sha1  $work/archive.zip" | sha1sum -c --quiet - || { echo "the archive's SHA-1 differs from the index" >&2; exit 1; }
[[ "$(stat -c %s "$work/archive.zip")" == "$size" ]] || { echo "the archive's size differs from the index" >&2; exit 1; }
sha256="$(sha256sum "$work/archive.zip" | cut -d' ' -f1)"

if [[ "${1:-}" == "--check" ]]; then
  ok=1
  [[ "$current" == "$version" ]] || { echo "OUTDATED: Google's newest is $version, $current is pinned" >&2; ok=0; }
  [[ "$(pinned URL)" == "$base/$archive" ]] || { echo "CHANGED: the URL is now $base/$archive" >&2; ok=0; }
  pinned_size="$(pinned SIZE)"; pinned_size="${pinned_size//_/}"
  [[ "$pinned_size" == "$size" ]] || { echo "CHANGED: the index says $size bytes, $pinned_size are pinned" >&2; ok=0; }
  [[ "$(pinned SHA256)" == "$sha256" ]] || { echo "CHANGED: the archive's SHA-256 is $sha256, $(pinned SHA256) is pinned" >&2; ok=0; }
  if (( ok )); then
    echo "The pin is Google's current archive (size, SHA-1 of the index and SHA-256 checked)."
    exit 0
  fi
  echo "Every \"Installer adb\" download is refused: pin again (this script without --check), check the signatures on Windows, release." >&2
  exit 1
fi

python3 -I - "$file" "$version" "$base/$archive" "$sha256" "$size" <<'PY'
import re, sys
path, version, url, sha256, size = sys.argv[1:]
text = open(path, encoding="utf-8").read()
size_rust = f"{int(size):_}"
for pattern, value in [
    (r'(pub const VERSION: &str = ")[^"]*(";)', version),
    (r'(pub const URL: &str = ")[^"]*(";)', url),
    (r'(pub const SHA256: &str = ")[^"]*(";)', sha256),
    (r"(pub const SIZE: u64 = )[0-9_]+(;)", size_rust),
]:
    text, n = re.subn(pattern, lambda m: m.group(1) + value + m.group(2), text)
    assert n == 1, pattern
open(path, "w", encoding="utf-8").write(text)
PY
echo "Pinned $version: SHA-256 $sha256, $size bytes (SHA-1 checked against Google's index)."
