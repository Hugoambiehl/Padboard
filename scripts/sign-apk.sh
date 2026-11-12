#!/usr/bin/env bash
# Signs an unsigned release APK (from the release workflow) with the tablet's
# release key, on this PC: the key never goes to CI.
#
#   scripts/sign-apk.sh padboard-0.1.0-unsigned.apk      -> padboard-0.1.0.apk
#
# The key is found like Gradle does (docs/android-release.md): the
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

prop() { grep -E "^$1=" "$2" | tail -n1 | cut -d= -f2-; }
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

sdk="${ANDROID_HOME:-$HOME/Android/Sdk}"
tools="$(find "$sdk/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n1)"
[[ -x "$tools/apksigner" ]] || { echo "error: apksigner not found under $sdk/build-tools" >&2; exit 1; }

# The APK must be aligned before signing (AGP already aligns it).
"$tools/zipalign" -c -P 16 4 "$unsigned" || { echo "error: $unsigned is not aligned" >&2; exit 1; }

"$tools/apksigner" sign \
  --ks "$store" --ks-key-alias "$alias" \
  --ks-pass env:PADBOARD_STORE_PASS --key-pass env:PADBOARD_KEY_PASS \
  --out "$signed" "$unsigned"
rm -f "$signed.idsig"
"$tools/apksigner" verify --print-certs "$signed" | grep -E "Signer #1 certificate (DN|SHA-256)"
echo "Signed: $signed"
