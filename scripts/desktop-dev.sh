#!/usr/bin/env bash
# Runs the desktop app in dev mode (`npm run tauri dev` in apps/desktop).
#
#   scripts/desktop-dev.sh                     loopback only (default, USB via adb reverse)
#   PADBOARD_BIND=lan scripts/desktop-dev.sh   also listen on the local network
#
# The environment leaked by snap terminals (VS Code snap) is cleaned first,
# see lib/clean-snap-env.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/clean-snap-env.sh
source "$ROOT/scripts/lib/clean-snap-env.sh"

cd "$ROOT/apps/desktop"
exec npm run tauri dev -- "$@"
