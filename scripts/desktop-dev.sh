#!/usr/bin/env bash
# Runs the desktop app in dev mode (`npm run tauri dev` in apps/desktop).
#
#   scripts/desktop-dev.sh                     loopback only (default, USB via adb reverse)
#   PADBOARD_BIND=lan scripts/desktop-dev.sh   also listen on the local network
#
# The environment leaked by snap terminals (VS Code snap) is cleaned first,
# see lib/clean-snap-env.sh. Without PADBOARD_TEST_DIR it warns that this
# development version runs on the real configuration.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/clean-snap-env.sh
source "$ROOT/scripts/lib/clean-snap-env.sh"

# A build of main is a development version (X.Y.Z-dev). On the real
# configuration, tablets running the published app may not connect to it
# (another protocol), and the installed Padboard would fight it for the
# same ports.
version="$(node -p 'require("'"$ROOT"'/apps/desktop/src-tauri/tauri.conf.json").version')"
if [[ -z "${PADBOARD_TEST_DIR:-}" ]]; then
  echo "desktop-dev: development version $version on your REAL configuration." >&2
  echo "desktop-dev: tablets running the published Padboard may not connect to it (protocol);" >&2
  echo "desktop-dev: to test with a tablet, set PADBOARD_TEST_DIR (docs/development.md)," >&2
  echo "desktop-dev: to use Padboard, start the installed version instead." >&2
  if pgrep -x padboard-deskto >/dev/null; then
    echo "desktop-dev: the installed Padboard is running: quit it first (same ports)." >&2
  fi
fi

cd "$ROOT/apps/desktop"
exec npm run tauri dev -- "$@"
