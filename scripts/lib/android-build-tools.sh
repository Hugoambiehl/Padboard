# shellcheck shell=bash
# Sets `build_tools` to the newest Android SDK build-tools folder
# (ANDROID_HOME, or ~/Android/Sdk), or fails with a message. Sourced by
# scripts/sign-apk.sh and scripts/sign-release.sh.
android_sdk="${ANDROID_HOME:-$HOME/Android/Sdk}"
build_tools="$(find "$android_sdk/build-tools" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n1)"
if [[ ! -x "$build_tools/apksigner" ]]; then
  echo "error: apksigner not found under $android_sdk/build-tools" >&2
  exit 1
fi
