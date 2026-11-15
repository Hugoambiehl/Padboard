#!/usr/bin/env bash
# Runs the tablet app's UI tests (apps/android/app/src/androidTest, e.g.
# KioskCornerTest: nothing on any screen in the corner that leaves kiosk
# mode) on a throwaway emulator at the SM-T580's size (1920 × 1200, 240 dpi
# = 1280 × 800 dp). Local only (not in CI).
#
#   scripts/tablet-ui-tests.sh
#
# The emulator lives in a temporary folder (ANDROID_AVD_HOME), never next
# to your own AVDs, and is stopped and deleted at the end. System image: the
# first x86_64 one installed, or PADBOARD_TEST_IMAGE
# (e.g. "system-images;android-37.0;google_apis_playstore_ps16k;x86_64").
#
# The API 37 images abort on some screen reads ("Assertion failed:
# !rcEnc->featureInfo()->hasReadColorBufferDma"): SurfaceFlinger when the
# launcher samples the screen, system_server when it snapshots a task
# switch. Hence `-gpu guest`, the launcher turned off, and every screen
# checked within one activity. Screenshots (screencap) fail the same way.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
avdmanager="$sdk/cmdline-tools/latest/bin/avdmanager"
emulator="$sdk/emulator/emulator"
port=5580
serial="emulator-$port"
[[ -x "$avdmanager" && -x "$emulator" ]] || { echo "error: no avdmanager or emulator in $sdk" >&2; exit 2; }

image="${PADBOARD_TEST_IMAGE:-}"
if [[ -z "$image" ]]; then
  dir="$(find "$sdk/system-images" -mindepth 3 -maxdepth 3 -type d -name x86_64 2>/dev/null | sort | tail -1)"
  [[ -n "$dir" ]] || { echo "error: no x86_64 system image in $sdk/system-images" >&2; exit 2; }
  image="system-images;$(realpath --relative-to="$sdk/system-images" "$dir" | tr '/' ';')"
fi
if adb devices | grep -q "^$serial"; then
  echo "error: $serial is already running" >&2
  exit 2
fi
# The kernel keeps 15 characters of a process name.
if pgrep -x padboard-deskto > /dev/null; then
  echo "note: Padboard runs on this PC: it will see the emulator like a tablet (adb reverse)." >&2
  echo "      The tests never start the app's connection, so nothing more happens." >&2
fi

work="$(mktemp -d)"
emulator_pid=""
cleanup() {
  if [[ -n "$emulator_pid" ]]; then
    adb -s "$serial" emu kill > /dev/null 2>&1 || true
    wait "$emulator_pid" 2> /dev/null || true
  fi
  rm -rf -- "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP

export ANDROID_AVD_HOME="$work" ANDROID_SERIAL="$serial"
echo "== Emulator ($image)"
echo no | "$avdmanager" -s create avd -n PadboardUiTests -k "$image" -p "$work/PadboardUiTests.avd" > /dev/null 2>&1
config="$work/PadboardUiTests.avd/config.ini"
sed -i '/^hw\.lcd\.\(width\|height\|density\)=/d;/^hw\.initialOrientation=/d;/^hw\.ramSize=/d' "$config"
printf '%s\n' hw.lcd.width=1920 hw.lcd.height=1200 hw.lcd.density=240 \
  hw.initialOrientation=landscape hw.ramSize=2048 >> "$config"
"$emulator" -avd PadboardUiTests -port "$port" -no-window -no-audio -no-boot-anim \
  -no-snapshot -gpu guest > "$work/emulator.log" 2>&1 &
emulator_pid=$!

echo "== Build"
(cd "$root/apps/android" && ./gradlew -q assembleDebug assembleDebugAndroidTest)

echo "== Waiting for the emulator to start"
timeout 300 adb wait-for-device
timeout 300 adb shell 'while [ "$(getprop sys.boot_completed)" != 1 ]; do sleep 2; done; until pm path android > /dev/null 2>&1; do sleep 2; done'
adb shell pm disable-user --user 0 com.google.android.apps.nexuslauncher > /dev/null 2>&1 || true
# Let the system settle without the launcher (right after boot, it still crashed).
sleep 20
adb shell wm size

echo "== Tests"
outputs="$root/apps/android/app/build/outputs/apk"
adb install -r -t "$outputs/debug/app-debug.apk" > /dev/null
adb install -r -t "$outputs/androidTest/debug/app-debug-androidTest.apk" > /dev/null
result="$(adb shell am instrument -w io.github.hugoambiehl.padboard.test/androidx.test.runner.AndroidJUnitRunner)"
echo "$result"
adb logcat -d -s KioskCornerTest:I | grep -v '^-' || true
if ! grep -q '^OK (' <<< "$result"; then
  adb logcat -d -b crash | grep -m3 'Abort message' >&2 || true
  exit 1
fi
