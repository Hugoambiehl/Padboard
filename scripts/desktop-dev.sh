#!/usr/bin/env bash
# Runs the desktop app in dev mode (`npm run tauri dev` in apps/desktop).
#
#   scripts/desktop-dev.sh                 loopback only (default, USB via adb reverse)
#   PADBOARD_BIND=lan scripts/desktop-dev.sh   also listen on the local network
#
# When started from the terminal of an app installed as a snap (e.g. VS Code),
# the environment carries the snap's GTK/GIO paths. WebKitGTK then loads the
# snap's GSettings schemas instead of the system ones and aborts with
# "Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not
# contain a key named 'antialiasing'". Those variables are removed here.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -n "${SNAP:-}" ]]; then
  unset GSETTINGS_SCHEMA_DIR GTK_PATH GTK_EXE_PREFIX GTK_IM_MODULE_FILE GIO_MODULE_DIR \
    GDK_PIXBUF_MODULEDIR GDK_PIXBUF_MODULE_FILE LOCPATH
  if [[ -n "${XDG_DATA_DIRS_VSCODE_SNAP_ORIG:-}" ]]; then
    # The VS Code snap keeps the original value aside.
    export XDG_DATA_DIRS="$XDG_DATA_DIRS_VSCODE_SNAP_ORIG"
  elif [[ -n "${XDG_DATA_DIRS:-}" ]]; then
    # Otherwise drop the snap's own entries and keep the rest.
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
  echo "desktop-dev: removed the snap's GTK/GIO environment (started from $SNAP)." >&2
fi

cd "$ROOT/apps/desktop"
exec npm run tauri dev -- "$@"
