# Sourced: writes one version everywhere (shared by release.sh and
# dev-version.sh). Android's versionCode must always grow:
#   MAJOR*1000000 + MINOR*10000 + PATCH*100 + N
# with N = rc number for X.Y.Z-rc.N, 99 for the release X.Y.Z, 0 for the
# development version X.Y.Z-dev that main carries between releases (below
# every release candidate of X.Y.Z, above every earlier release).

# Parses $1; sets version_code, or fails with a message.
parse_version() {
  local version="$1"
  if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-rc\.([0-9]+)|-dev)?$ ]]; then
    echo "error: $version is not MAJOR.MINOR.PATCH[-rc.N|-dev]" >&2
    return 2
  fi
  local major="${BASH_REMATCH[1]}" minor="${BASH_REMATCH[2]}" patch="${BASH_REMATCH[3]}"
  local suffix="${BASH_REMATCH[4]}" rc="${BASH_REMATCH[5]:-}"
  if [[ -n "$rc" ]] && ((rc < 1 || rc > 98)); then
    echo "error: release candidates go from rc.1 to rc.98" >&2
    return 2
  fi
  if ((minor > 99 || patch > 99)); then
    echo "error: minor and patch must stay below 100 (Android versionCode)" >&2
    return 2
  fi
  local n=99
  [[ -n "$rc" ]] && n="$rc"
  [[ "$suffix" == -dev ]] && n=0
  version_code=$((major * 1000000 + minor * 10000 + patch * 100 + n))
}

# Writes $1 (already parsed: $version_code set) into every file; the caller commits.
write_version() {
  local version="$1"
  # Workspace crates (Cargo.toml [workspace.package]) and Cargo.lock.
  sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = ".*"/version = "'"$version"'"/' Cargo.toml
  cargo update -q --workspace
  # Desktop app: Tauri (installer and window) and the frontend package.
  node -e '
    const fs = require("fs");
    const [version] = process.argv.slice(1);
    for (const f of ["apps/desktop/src-tauri/tauri.conf.json", "apps/desktop/package.json"]) {
      const json = JSON.parse(fs.readFileSync(f, "utf8"));
      json.version = version;
      fs.writeFileSync(f, JSON.stringify(json, null, 2) + "\n");
    }
    const lockFile = "apps/desktop/package-lock.json";
    const lock = JSON.parse(fs.readFileSync(lockFile, "utf8"));
    lock.version = version;
    lock.packages[""].version = version;
    fs.writeFileSync(lockFile, JSON.stringify(lock, null, 2) + "\n");
  ' "$version"
  # Tablet app.
  sed -i -E \
    -e 's/^(\s*versionCode = ).*/\1'"$version_code"'/' \
    -e 's/^(\s*versionName = ).*/\1"'"$version"'"/' \
    apps/android/app/build.gradle.kts
}

VERSION_FILES=(Cargo.toml Cargo.lock
  apps/desktop/src-tauri/tauri.conf.json apps/desktop/package.json apps/desktop/package-lock.json
  apps/android/app/build.gradle.kts)

# The development version that follows the release $1: the same X.Y.Z
# after a release candidate (the release is still to come), X.(Y+1).0
# after the release itself.
next_dev_version() {
  local version="$1"
  [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-rc\.[0-9]+)?$ ]] || return 2
  if [[ -n "${BASH_REMATCH[4]}" ]]; then
    echo "${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}-dev"
  else
    echo "${BASH_REMATCH[1]}.$((BASH_REMATCH[2] + 1)).0-dev"
  fi
}
