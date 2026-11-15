#!/usr/bin/env bash
# Signs a draft release on this PC: the keys never go to CI.
#
#   scripts/sign-release.sh 0.2.0              # the draft on GitHub: download, sign, upload
#   scripts/sign-release.sh 0.2.0 --dir DIR    # the files of DIR only, nothing sent (tests)
#
# 1. checks the files against the CI's SHA256SUMS;
# 2. signs the tablet APK (scripts/sign-apk.sh) and checks its certificate;
# 3. signs the Windows installer for the in-app updater (minisign, bound to
#    the version) and writes latest.json;
# 4. writes SHA256SUMS again (with the signed APK, the .sig, latest.json)
#    and signs it too: the PC checks it before sending the APK to a tablet;
# 5. checks everything with the app's own code (example verify-release),
#    then uploads (draft releases only: a published release is never re-signed).
#
# Authenticode (SignPath): once the release workflow signs the installer,
# it does so before the draft is created, so the installer downloaded here is
# the final one and the minisign signature covers it. The script then
# refuses an installer without an Authenticode signature when the workflow
# has a SignPath step (`uses: signpath/...`). Nothing may change the installer after this script.
#
# Keys (docs/release-signing.md): PADBOARD_UPDATER_KEY (default
# ~/.padboard-signing/updater.key, password required), the APK's through
# scripts/sign-apk.sh. PADBOARD_UPDATER_PUB overrides the public key checked
# against (default: the one embedded in the app, for tests only).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="Hugoambiehl/Padboard"
apk_certificate="637bb3c07a1bc2a0c10d92f4152ef9c92fbf00c9d510b3e72f63f7c2e3dcfa46"

die() { echo "error: $*" >&2; exit 1; }

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]]; then
  echo "usage: $0 MAJOR.MINOR.PATCH[-rc.N] [--dir DIR]" >&2
  exit 2
fi
tag="v$version"
local_dir=""
if [[ "${2:-}" == "--dir" ]]; then
  local_dir="${3:?--dir needs a folder}"
  [[ -d "$local_dir" ]] || die "$local_dir is not a folder"
fi

key="${PADBOARD_UPDATER_KEY:-$HOME/.padboard-signing/updater.key}"
pub="${PADBOARD_UPDATER_PUB:-$root/apps/desktop/src-tauri/updater-key.pub}"
[[ -s "$pub" ]] || die "no update public key in $pub (docs/release-signing.md)"
[[ -f "$key" ]] || die "update key not found: $key (docs/release-signing.md)"
command -v python3 >/dev/null || die "python3 is needed"

tauri() { (cd "$root/apps/desktop" && npx --no-install tauri "$@"); }

if [[ -n "$local_dir" ]]; then
  work="$local_dir"
else
  command -v gh >/dev/null || die "the GitHub CLI (gh) is needed"
  [[ "$(gh release view "$tag" --repo "$repo" --json isDraft -q .isDraft)" == "true" ]] \
    || die "$tag is not a draft release: a published release is never re-signed"
  # Removed on every exit: success, error (die) or interruption. The traps
  # come first, so a signal right after mktemp still removes the folder.
  work=""
  trap '[[ -z "$work" ]] || rm -rf -- "$work"' EXIT
  trap 'exit 130' INT TERM HUP
  work="$(mktemp -d)"
  gh release download "$tag" --repo "$repo" -D "$work"
fi

[[ ! -e "$work/SHA256SUMS.sig" ]] || die "$tag is already signed"
echo "== Checking the files built by CI"
(cd "$work" && sha256sum --quiet -c SHA256SUMS) || die "a file does not match the CI's SHA256SUMS"

installer="Padboard_${version}_x64-setup.exe"
apk="padboard-$version.apk"
unsigned="padboard-$version-unsigned.apk"
[[ -f "$work/$installer" ]] || die "$installer is missing"

# Has the installer an Authenticode signature (PE security directory)?
authenticode="$(python3 -I - "$work/$installer" <<'EOF'
import struct, sys
with open(sys.argv[1], "rb") as f:
    data = f.read(4096)
pe = struct.unpack_from("<I", data, 0x3C)[0]
if data[pe:pe + 4] != b"PE\0\0":
    sys.exit("not a PE file")
optional = pe + 24
magic = struct.unpack_from("<H", data, optional)[0]
directories = optional + (96 if magic == 0x10B else 112)
size = struct.unpack_from("<I", data, directories + 4 * 8 + 4)[0]
print("signed" if size > 0 else "unsigned")
EOF
)"
if grep -qiE '^\s*(-\s*)?uses:\s*signpath/' "$root/.github/workflows/release.yml"; then
  [[ "$authenticode" == "signed" ]] || die "$installer has no Authenticode signature, but the release workflow signs it (SignPath)"
fi
echo "Installer: Authenticode $authenticode"

# The update key's password, asked and checked before anything is signed:
# end of input (Ctrl+D), an empty or a wrong password stop here, clearly.
echo "== Password of the update key ($key)"
probe="$work/.key-probe"
echo probe > "$probe"
probe_sign() { # password: signs the probe with it, quietly
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$1" tauri signer sign -f "$key" "$probe" </dev/null >/dev/null 2>&1
}
if probe_sign ""; then
  rm -f -- "$probe" "$probe.sig"
  die "$key has no password: generate it again with one (docs/release-signing.md)"
fi
if ! read -rsp "Password: " TAURI_SIGNING_PRIVATE_KEY_PASSWORD; then
  echo >&2
  rm -f -- "$probe" "$probe.sig"
  die "no password entered (end of input, Ctrl+D?): nothing signed, nothing uploaded"
fi
echo
if [[ -z "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ]]; then
  rm -f -- "$probe" "$probe.sig"
  die "empty password: the update key has one (nothing signed, nothing uploaded)"
fi
if ! probe_sign "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"; then
  rm -f -- "$probe" "$probe.sig"
  die "wrong password for $key: nothing signed, nothing uploaded"
fi
rm -f -- "$probe" "$probe.sig"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

echo "== Tablet APK"
if [[ ! -f "$work/$apk" ]]; then
  [[ -f "$work/$unsigned" ]] || die "$unsigned is missing"
  "$root/scripts/sign-apk.sh" "$work/$unsigned"
fi
# Exactly one signer, with the release certificate (apksigner's output of
# build-tools <= 36 and >= 37: scripts/lib/apk_certificate.py, tested).
# shellcheck source=lib/android-build-tools.sh
source "$root/scripts/lib/android-build-tools.sh"
certs="$("$build_tools/apksigner" verify --print-certs "$work/$apk")" \
  || die "$apk does not verify (apksigner)"
python3 -I "$root/scripts/lib/apk_certificate.py" "$apk_certificate" <<<"$certs" \
  || die "$apk is not signed with the release key only ($apk_certificate)"
rm -f -- "$work/$unsigned"

echo "== Update signatures"
sign_update() { # file: signed for $version with the update key, or a clear error
  local errors="$work/.sign-errors"
  if ! tauri signer sign --app-version "$version" -f "$key" "$1" >/dev/null 2>"$errors"; then
    sed 's/^/  /' "$errors" >&2
    rm -f -- "$errors"
    die "cannot sign $(basename "$1") with $key (nothing uploaded)"
  fi
  rm -f -- "$errors"
}
sign_update "$work/$installer"

python3 -I - "$work" "$version" "$installer" "$repo" <<'EOF'
import datetime, json, os, sys
work, version, installer, repo = sys.argv[1:]
with open(os.path.join(work, installer + ".sig")) as f:
    signature = f.read().strip()
entry = {
    "signature": signature,
    "url": f"https://github.com/{repo}/releases/download/v{version}/{installer}",
}
manifest = {
    "version": version,
    "notes": f"https://github.com/{repo}/releases/tag/v{version}",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"windows-x86_64": entry, "windows-x86_64-nsis": entry},
}
with open(os.path.join(work, "latest.json"), "w") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
EOF

(
  cd "$work"
  find . -maxdepth 1 -type f ! -name 'SHA256SUMS*' ! -name '.*' -printf '%f\n' | sort \
    | xargs -d '\n' sha256sum -- > SHA256SUMS
)
sign_update "$work/SHA256SUMS"
unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD

echo "== Checking with the app's code"
(cd "$root" && cargo run -q -p padboard-desktop --example verify-release -- "$work" "$version" --key "$pub")

if [[ -n "$local_dir" ]]; then
  echo "Signed in $local_dir (nothing uploaded)."
  exit 0
fi

echo "== Uploading to the draft $tag"
gh release upload "$tag" --repo "$repo" --clobber \
  "$work/$apk" "$work/$installer.sig" "$work/latest.json" "$work/SHA256SUMS" "$work/SHA256SUMS.sig"
if gh release view "$tag" --repo "$repo" --json assets -q '.assets[].name' | grep -qx "$unsigned"; then
  gh release delete-asset "$tag" "$unsigned" --repo "$repo" -y
fi
echo "Draft $tag signed. Test it, then publish it (docs/releasing.md)."
