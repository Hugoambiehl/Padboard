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
#
# Gradle caches the absence of a module: once a repository failed, the same
# machine keeps reporting the plugin missing without asking again (every
# retry failed on the runner that saw the first failure, a fresh runner
# passed). Retries therefore add --refresh-dependencies. They also add
# --info, whose HTTP lines are the only place Gradle names the real failure
# (status code, timeout): those are printed, the whole output is folded.
set -uo pipefail

[[ $# -gt 0 ]] || { echo "usage: $0 <gradle command…>" >&2; exit 2; }
attempts="${GRADLE_ATTEMPTS:-3}"
pause="${GRADLE_RETRY_PAUSE:-60}"
log="$(mktemp)"
trap 'rm -f -- "$log"' EXIT

# What Gradle prints when a repository failed it (not the build itself).
unreachable='was not found in any of the following sources|Could not resolve|Could not GET|Could not HEAD|Received status code (429|5[0-9][0-9])|Read timed out|Connect timed out|Connection reset|Remote host terminated the handshake'
# --info lines about the requests themselves.
http='Received status code|Could not (GET|HEAD)|status code [0-9]{3}|timed out|Connection reset|terminated the handshake|Too Many Requests|Failed to get resource|Resource missing'

run() { # attempt command…
  local attempt="$1"
  shift
  if ((attempt == 1)); then
    "$@" 2>&1 | tee "$log"
    return "${PIPESTATUS[0]}"
  fi
  echo "::group::Gradle output (attempt $attempt, --refresh-dependencies --info)"
  "$@" --refresh-dependencies --info 2>&1 | tee "$log"
  local status=${PIPESTATUS[0]}
  echo "::endgroup::"
  if ((status != 0)); then
    echo "gradle-retry: requests that failed (attempt $attempt):"
    grep -E "$http" "$log" | sort | uniq -c | sort -rn | head -n 40
  fi
  return "$status"
}

for ((attempt = 1; ; attempt++)); do
  run "$attempt" "$@"
  status=$?
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
