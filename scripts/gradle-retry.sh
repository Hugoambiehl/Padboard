#!/usr/bin/env bash
# Runs a Gradle command (CI), and runs it again when it failed only because
# a repository did not answer: Maven Central rate-limits GitHub's runners
# (429) and the Gradle Plugin Portal has short outages; Gradle then stops
# with "Plugin … was not found" or "Could not resolve …" although nothing
# is wrong with the build (#22). A failing build or test is never retried.
#
#   scripts/gradle-retry.sh ./gradlew assembleRelease
#
# Up to GRADLE_ATTEMPTS attempts (default 3), GRADLE_RETRY_PAUSE seconds
# apart (default 60, doubled each time).
set -uo pipefail

[[ $# -gt 0 ]] || { echo "usage: $0 <gradle command…>" >&2; exit 2; }
attempts="${GRADLE_ATTEMPTS:-3}"
pause="${GRADLE_RETRY_PAUSE:-60}"
log="$(mktemp)"
trap 'rm -f -- "$log"' EXIT

# What Gradle prints when a repository failed it (not the build itself).
unreachable='was not found in any of the following sources|Could not resolve|Could not GET|Could not HEAD|Received status code (429|5[0-9][0-9])|Read timed out|Connect timed out|Connection reset|Remote host terminated the handshake'

for ((attempt = 1; ; attempt++)); do
  "$@" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  ((status == 0)) && exit 0
  if ! grep -qE "$unreachable" "$log"; then
    exit "$status"
  fi
  if ((attempt >= attempts)); then
    echo "gradle-retry: a repository still did not answer after $attempts attempts" >&2
    exit "$status"
  fi
  echo "gradle-retry: a repository did not answer (attempt $attempt of $attempts): again in ${pause} s" >&2
  echo "::warning::Gradle: a repository did not answer, retrying (attempt $((attempt + 1)) of $attempts)"
  sleep "$pause"
  pause=$((pause * 2))
done
