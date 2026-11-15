#!/usr/bin/env bash
# Signs an unsigned release APK (from the release workflow) with the tablet's
# release key, on this PC: the key never goes to CI.
#
#   scripts/sign-apk.sh padboard-0.1.0-unsigned.apk      -> padboard-0.1.0.apk
#
# The key is found like Gradle does (docs/release-signing.md): the
# `padboard.signing` property of ~/.gradle/gradle.properties, or
# PADBOARD_SIGNING, names a properties file with storeFile, keyAlias,
# storePassword (and optionally keyPassword). Passwords go to apksigner
# through environment variables, never on the command line.
# Needs the Android SDK build-tools (ANDROID_HOME, or ~/Android/Sdk).
set -euo pipefail

unsigned="${1:-}"
if [[ ! -f "$unsigned" || "$unsigned" != *-unsigned.apk ]]; then
  echo "usage: $0 <padboard-X-unsigned.apk>" >&2
  exit 2
fi
signed="${unsigned%-unsigned.apk}.apk"

# A property's value, empty when absent (keyPassword is optional).
prop() { { grep -E "^$1=" "$2" || true; } | tail -n1 | cut -d= -f2-; }
signing="${PADBOARD_SIGNING:-}"
if [[ -z "$signing" && -f "$HOME/.gradle/gradle.properties" ]]; then
  signing="$(prop padboard.signing "$HOME/.gradle/gradle.properties")"
fi
signing="${signing/#\~/$HOME}"
[[ -f "$signing" ]] || { echo "error: signing properties not found (padboard.signing / PADBOARD_SIGNING)" >&2; exit 1; }

store="$(prop storeFile "$signing")"
store="${store/#\~/$HOME}"
alias="$(prop keyAlias "$signing")"
export PADBOARD_STORE_PASS PADBOARD_KEY_PASS
PADBOARD_STORE_PASS="$(prop storePassword "$signing")"
PADBOARD_KEY_PASS="$(prop keyPassword "$signing")"
PADBOARD_KEY_PASS="${PADBOARD_KEY_PASS:-$PADBOARD_STORE_PASS}"

# shellcheck source=lib/android-build-tools.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/android-build-tools.sh"
tools="$build_tools"

# The APK must be aligned before signing (AGP already aligns it).
"$tools/zipalign" -c -P 16 4 "$unsigned" || { echo "error: $unsigned is not aligned" >&2; exit 1; }

"$tools/apksigner" sign \
  --ks "$store" --ks-key-alias "$alias" \
  --ks-pass env:PADBOARD_STORE_PASS --key-pass env:PADBOARD_KEY_PASS \
  --out "$signed" "$unsigned"
rm -f "$signed.idsig"
# Every signer's DN and SHA-256 (the caller checks them: sign-release.sh).
certs="$("$tools/apksigner" verify --print-certs "$signed")"
grep -E "certificate (DN|SHA-256 digest):" <<<"$certs" || true
echo "Signed: $signed"
