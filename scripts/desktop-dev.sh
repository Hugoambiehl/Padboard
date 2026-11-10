#!/usr/bin/env bash
# Runs the desktop app in dev mode (`npm run tauri dev` in apps/desktop).
#
#   scripts/desktop-dev.sh                     loopback only (default, USB via adb reverse)
#   PADBOARD_BIND=lan scripts/desktop-dev.sh   also listen on the local network
#
# When started from the terminal of an app installed as a snap (e.g. VS Code),
# the environment carries the snap's GTK/GIO paths. WebKitGTK then loads the
# snap's GSettings schemas instead of the system ones and aborts with
# "Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not
# contain a key named 'antialiasing'". The original environment is restored
# here. From a normal terminal, nothing is changed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# 1. The VS Code snap saves each variable X it overrides as X_VSCODE_SNAP_ORIG
#    (empty when X was not set). Restore X from it, or unset X.
restored=()
while IFS= read -r orig; do
  name="${orig%_VSCODE_SNAP_ORIG}"
  if [[ -n "${!orig}" ]]; then
    export "$name=${!orig}"
  else
    unset "$name"
  fi
  restored+=("$name")
done < <(compgen -e | grep '_VSCODE_SNAP_ORIG$' || true)
if ((${#restored[@]} > 0)); then
  echo "desktop-dev: restored the environment saved by the VS Code snap: ${restored[*]}" >&2
fi

# 2. Fallback for snap shells that do not save every original value (e.g. a
#    process spawned by the snap itself): drop the variables that still point
#    into the snap.
if [[ -n "${SNAP:-}" ]]; then
  for name in GSETTINGS_SCHEMA_DIR GTK_PATH GTK_EXE_PREFIX GTK_IM_MODULE_FILE GIO_MODULE_DIR \
    GDK_PIXBUF_MODULEDIR GDK_PIXBUF_MODULE_FILE LOCPATH; do
    case "${!name:-}" in
      "$SNAP"* | "$HOME/snap/"*) unset "$name" ;;
    esac
  done
  if [[ -n "${XDG_DATA_DIRS:-}" ]]; then
    cleaned=""
    IFS=':' read -ra dirs <<< "$XDG_DATA_DIRS"
    for dir in "${dirs[@]}"; do
      case "$dir" in
        "$SNAP"* | "$HOME/snap/"*) ;;
        *) cleaned="${cleaned:+$cleaned:}$dir" ;;
      esac
    done
    export XDG_DATA_DIRS="${cleaned:-/usr/local/share:/usr/share}"
  fi
fi

cd "$ROOT/apps/desktop"
exec npm run tauri dev -- "$@"
