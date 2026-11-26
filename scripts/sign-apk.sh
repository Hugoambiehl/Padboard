#!/usr/bin/env bash
# Signs an unsigned release APK (from the release workflow) with the tablet's
# release key, on this PC: the key never goes to CI.
#
#   scripts/sign-apk.sh padboard-0.1.0-unsigned.apk      -> padboard-0.1.0.apk
#
# The key (docs/release-signing.md): PADBOARD_SIGNING, else the `padboard.signing` property of
# ~/.gradle/gradle.properties, else ~/.padboard-signing/signing.properties,
# names a properties file with storeFile, keyAlias, storePassword (and
# optionally keyPassword). Passwords go to apksigner
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
default_signing="$HOME/.padboard-signing/signing.properties"
signing="${PADBOARD_SIGNING:-}"
from="PADBOARD_SIGNING"
if [[ -z "$signing" && -f "$HOME/.gradle/gradle.properties" ]]; then
  signing="$(prop padboard.signing "$HOME/.gradle/gradle.properties")"
  from="padboard.signing in ~/.gradle/gradle.properties"
fi
# The documented place: found even when gradle.properties lost its line
# (rewritten on 2026-11-26, the 0.2.2-rc.1 signing failed).
if [[ -z "$signing" ]]; then
  signing="$default_signing"
  from="the default location"
fi
signing="${signing/#\~/$HOME}"
if [[ ! -f "$signing" ]]; then
  {
    echo "error: signing properties not found: $signing (from $from)"
    echo "Point to them with, for example:"
    echo "  PADBOARD_SIGNING=\"\$HOME/.padboard-signing/signing.properties\" scripts/sign-release.sh <version>"
    echo "(docs/release-signing.md, \"Tablet APK key\")"
  } >&2
  exit 1
fi
echo "Signing properties: $signing (from $from)"

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
