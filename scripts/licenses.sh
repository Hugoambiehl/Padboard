#!/usr/bin/env bash
# Third-party licenses of the desktop app, for its "Licences" screen:
# Rust crates (cargo-about, checked against about.toml) and the npm packages
# of the frontend (scripts/licenses.mjs), written to
# apps/desktop/src/generated/licenses.json (not committed: built by CI and
# the release workflow; development builds show an empty list).
#
#   scripts/licenses.sh
#
# Needs cargo-about (version below) and `npm ci` in apps/desktop.
set -euo pipefail

CARGO_ABOUT_VERSION="0.9.3"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/apps/desktop/src/generated/licenses.json"

if ! command -v cargo-about >/dev/null 2>&1; then
  echo "error: cargo-about not found. Install it with:" >&2
  echo "  cargo install cargo-about --version $CARGO_ABOUT_VERSION --features cli --locked" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --fail: any crate whose license is not accepted stops the build.
cargo about generate --format json --fail \
  -c "$ROOT/about.toml" \
  -m "$ROOT/apps/desktop/src-tauri/Cargo.toml" \
  >"$tmp/about.json"
node "$ROOT/scripts/licenses.mjs" "$tmp/about.json" "$OUT"
