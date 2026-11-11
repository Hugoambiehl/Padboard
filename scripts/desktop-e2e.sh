#!/usr/bin/env bash
# End-to-end tests of the desktop app: drives the real window through
# tauri-driver (WebKitWebDriver on Linux). Local only, not run in CI.
#
#   scripts/desktop-e2e.sh                     build, then run every e2e test
#   scripts/desktop-e2e.sh e2e/profiles        only the files matching a filter
#   scripts/desktop-e2e.sh --no-build ...      reuse the last build
#
# The app runs in test mode (PADBOARD_TEST_DIR, see src-tauri/src/test_mode.rs):
# temporary configuration, actions recorded and never injected, USB devices
# and Wi-Fi left alone. Your real configuration is never touched.
#
# Screenshots of the key steps: target/e2e/screenshots/.
#
# Environment:
#   PADBOARD_E2E_HEADLESS=1       run on a virtual X display (needs xvfb-run),
#                                 so the windows do not open on your screen
#   PADBOARD_E2E_NATIVE_DRIVER    path of WebKitWebDriver if not in PATH
#   PADBOARD_E2E_KEEP=1           keep the test directories (printed paths)
#
# Requirements (once): `sudo apt install webkit2gtk-driver` and
# `cargo install tauri-driver --locked`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/clean-snap-env.sh
source "$ROOT/scripts/lib/clean-snap-env.sh"

build=1
if [[ "${1:-}" == "--no-build" ]]; then
  build=0
  shift
fi

missing=0
if ! command -v tauri-driver > /dev/null; then
  echo "desktop-e2e: tauri-driver is missing: cargo install tauri-driver --locked" >&2
  missing=1
fi
if [[ -z "${PADBOARD_E2E_NATIVE_DRIVER:-}" ]] && ! command -v WebKitWebDriver > /dev/null; then
  echo "desktop-e2e: WebKitWebDriver is missing: sudo apt install webkit2gtk-driver" >&2
  missing=1
fi
runner=()
if [[ -n "${PADBOARD_E2E_HEADLESS:-}" ]]; then
  if ! command -v xvfb-run > /dev/null; then
    echo "desktop-e2e: PADBOARD_E2E_HEADLESS needs xvfb-run: sudo apt install xvfb" >&2
    missing=1
  fi
  runner=(xvfb-run --auto-servernum --server-args "-screen 0 1600x1000x24")
  export GDK_BACKEND=x11
fi
((missing == 0)) || exit 1

# A separate target directory: the e2e build embeds the frontend (no dev
# server), which would otherwise invalidate the `tauri dev` build each time.
export CARGO_TARGET_DIR="$ROOT/target/e2e"
cd "$ROOT/apps/desktop"
if ((build)); then
  npm run tauri build -- --debug --no-bundle
  cargo build -p padboard-desktop --example fake-tablet
fi

export PADBOARD_E2E_APP="$CARGO_TARGET_DIR/debug/padboard-desktop"
export PADBOARD_E2E_FAKE_TABLET="$CARGO_TARGET_DIR/debug/examples/fake-tablet"
export PADBOARD_E2E_SHOTS="$CARGO_TARGET_DIR/screenshots"
rm -rf "$PADBOARD_E2E_SHOTS"

"${runner[@]}" npx vitest run --config e2e/vitest.config.ts "$@"
echo "desktop-e2e: screenshots in $PADBOARD_E2E_SHOTS"
