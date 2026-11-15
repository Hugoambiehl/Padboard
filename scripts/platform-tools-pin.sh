#!/usr/bin/env bash
# Pins Google's Platform-Tools for Windows ("Installer adb") to the newest
# version of Google's repository index: version, URL, SHA-256 and size in
# apps/desktop/src-tauri/src/platform_tools/mod.rs. Google only publishes a
# SHA-1: the archive is downloaded, its SHA-1 checked against the index,
# then its SHA-256 computed and written.
#
#   scripts/platform-tools-pin.sh          # print the newest and update the file
#   scripts/platform-tools-pin.sh --check  # only say whether the pin is the newest
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
current="$(sed -n 's/^pub const VERSION: &str = "\(.*\)";/\1/p' "$file")"
echo "Pinned: $current; newest from Google: $version ($archive)"
if [[ "${1:-}" == "--check" ]]; then
  [[ "$current" == "$version" ]]
  exit
fi
[[ "$archive" =~ ^platform-tools_r[0-9.]+-win\.zip$ ]] || { echo "unexpected archive name: $archive" >&2; exit 1; }

curl -sSfL -o "$work/archive.zip" "$base/$archive"
echo "$sha1  $work/archive.zip" | sha1sum -c --quiet -
[[ "$(stat -c %s "$work/archive.zip")" == "$size" ]] || { echo "size differs from the index" >&2; exit 1; }
sha256="$(sha256sum "$work/archive.zip" | cut -d' ' -f1)"

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
