#!/usr/bin/env bash
# Installs a Padboard .deb in a clean Ubuntu container and starts the app,
# in test mode on a virtual display: the package must install with its
# dependencies, the app must start (no missing library or symbol) and the
# second screen must have an encoder (VP8 at least: Ubuntu 22.04's
# GStreamer 1.20 has no VA-API H.264 encoder).
#
#   scripts/deb-smoke-test.sh path/to/padboard.deb [ubuntu:22.04]
#
# Needs Docker. Used by CI (job "Linux package") and by hand.
set -euo pipefail

deb="$(realpath "${1:?usage: $0 <.deb> [image]}")"
image="${2:-ubuntu:22.04}"

docker run --rm -v "$deb:/tmp/padboard.deb:ro" "$image" bash -euo pipefail -c '
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
# What a user gets: Depends and Recommends, as apt installs them.
apt-get install -y -qq /tmp/padboard.deb xvfb xauth > /tmp/apt.log 2>&1 || { tail -30 /tmp/apt.log; exit 1; }
echo "installed: $(dpkg-query -W -f="\${Package} \${Version}" padboard)"
mkdir -p /tmp/test
export PADBOARD_TEST_DIR=/tmp/test PADBOARD_LOG=info
timeout 30 xvfb-run -a padboard-desktop > /tmp/out.log 2>&1 &
for _ in $(seq 30); do
  grep -q "second screen encoders" /tmp/test/app.log 2>/dev/null && break
  sleep 1
done
pkill -x padboard-deskto || true
if ! grep -q "WebSocket server listening" /tmp/test/app.log 2>/dev/null; then
  echo "the app did not start:"; tail -40 /tmp/out.log; cat /tmp/test/app.log 2>/dev/null; exit 1
fi
grep "second screen encoders" /tmp/test/app.log
grep "second screen encoders" /tmp/test/app.log | grep -q "Vp8" || { echo "no VP8 encoder"; exit 1; }
echo "OK: installed and started on '"$image"'"
'
