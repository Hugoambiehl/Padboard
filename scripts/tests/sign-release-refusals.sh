#!/usr/bin/env bash
# Local test of scripts/sign-release.sh's refusals (needs the Android SDK
# build-tools; not in CI): an APK signed with another key than the release
# key is refused before anything else, nothing is uploaded, and the
# temporary folder is removed, after an error as after an interruption.
#
#   scripts/tests/sign-release-refusals.sh
#
# Nothing real is used: a fake `gh` serves a fake draft (it never contacts
# GitHub and fails on any upload), the APK is signed with the Android debug
# key (~/.android/debug.keystore), never with ~/.padboard-signing.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
apk_in="$root/apps/android/app/build/outputs/apk/release/app-release-unsigned.apk"
[[ -f "$apk_in" ]] || { echo "build the unsigned release APK first: (cd apps/android && ./gradlew assembleRelease)" >&2; exit 2; }
[[ -f "$HOME/.android/debug.keystore" ]] || { echo "no ~/.android/debug.keystore" >&2; exit 2; }

test_dir="$(mktemp -d)"
trap 'rm -rf -- "$test_dir"' EXIT
version="9.9.9-rc.1"
draft="$test_dir/draft"
mkdir -p "$draft" "$test_dir/bin" "$test_dir/tmp"

# The fake draft: an unsigned APK, a minimal PE "installer", SHA256SUMS.
cp "$apk_in" "$draft/padboard-$version-unsigned.apk"
python3 -I - "$draft/Padboard_${version}_x64-setup.exe" <<'EOF'
import struct, sys
data = bytearray(512)
data[0:2] = b"MZ"
struct.pack_into("<I", data, 0x3C, 0x80)
data[0x80:0x84] = b"PE\0\0"
struct.pack_into("<H", data, 0x80 + 24, 0x20B)  # PE32+, no data directories set
open(sys.argv[1], "wb").write(data)
EOF
(cd "$draft" && sha256sum -- * > SHA256SUMS)

# A fake gh: the draft is a draft, downloads copy it, anything else fails
# (an upload would make the test fail).
cat > "$test_dir/bin/gh" <<EOF
#!/usr/bin/env bash
case "\$1 \$2" in
  "release view") echo true ;;
  "release download") dir="\${@: -1}"; cp -- "$draft"/* "\$dir"/ ;;
  *) echo "fake gh: unexpected call: \$*" >&2; echo "\$*" >> "$test_dir/unexpected"; exit 1 ;;
esac
EOF
chmod +x "$test_dir/bin/gh"

# The debug key, through the same properties file as the real one.
cat > "$test_dir/signing.properties" <<EOF
storeFile=$HOME/.android/debug.keystore
keyAlias=androiddebugkey
storePassword=android
EOF
touch "$test_dir/updater.key"

command=(env PATH="$test_dir/bin:$PATH" TMPDIR="$test_dir/tmp"
  PADBOARD_SIGNING="$test_dir/signing.properties"
  PADBOARD_UPDATER_KEY="$test_dir/updater.key"
  "$root/scripts/sign-release.sh" "$version")
run() { "${command[@]}" </dev/null; }

fail() { echo "FAIL: $*" >&2; exit 1; }

echo "== An APK signed with another key is refused"
if run > "$test_dir/out" 2>&1; then
  cat "$test_dir/out"
  fail "sign-release.sh accepted an APK signed with the debug key"
fi
grep -q "is not signed with the release key only" "$test_dir/out" || { cat "$test_dir/out"; fail "not refused for the certificate"; }
grep -q "signed with 5c10537c" "$test_dir/out" || { cat "$test_dir/out"; fail "the debug certificate was not read (apksigner output format?)"; }
[[ -z "$(ls -A "$test_dir/tmp")" ]] || fail "temporary folder left after the error: $(ls "$test_dir/tmp")"
[[ ! -e "$test_dir/unexpected" ]] || fail "unexpected gh calls: $(cat "$test_dir/unexpected")"
echo "ok: refused ($(grep -m1 'signed with' "$test_dir/out" | sed 's/^error: //')), nothing uploaded, no temporary folder left"

echo "== Interrupted: the temporary folder is removed too"
# exec: $! is sign-release.sh itself, not a subshell around it.
(exec "${command[@]}" </dev/null > "$test_dir/out-int" 2>&1) &
pid=$!
# Wait until it works on the downloaded draft (the files are in its
# temporary folder), then interrupt it.
for _ in $(seq 200); do
  compgen -G "$test_dir/tmp/*/SHA256SUMS" > /dev/null && break
  sleep 0.05
done
compgen -G "$test_dir/tmp/*/SHA256SUMS" > /dev/null || fail "the draft was never downloaded"
kill -TERM "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
[[ -z "$(ls -A "$test_dir/tmp")" ]] || fail "temporary folder left after an interruption: $(ls "$test_dir/tmp")"
echo "ok: no temporary folder left"
echo "All refusal tests passed."
