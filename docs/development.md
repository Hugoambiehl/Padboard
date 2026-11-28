# Development

How to build, run and test Padboard from the sources. The rules for changes
are in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Repository layout

| Path | Content |
|---|---|
| `crates/protocol` | Wire protocol: the source of truth, TypeScript and Kotlin types generated with typeshare |
| `crates/actions` | Action execution (`ActionExecutor`, Linux and Windows implementations) |
| `apps/desktop` | Desktop app (Tauri 2: Rust in `src-tauri`, Vue 3 + TypeScript in `src`) |
| `apps/android` | Tablet app (Kotlin, Jetpack Compose) |
| `assets/icon` | The app icon (`padboard.svg`); `node scripts/icons.mjs` generates every size |
| `docs/design` | Reference mockups of the UI |
| `scripts/` | Type generation, end-to-end tests, licenses, icons, releases |

## Desktop app (Linux)

Requirements: Rust (the version in `rust-toolchain.toml` is installed by
`rustup`), Node.js 24, and Tauri's system packages:

```sh
sudo apt install libwebkit2gtk-4.1-dev libdbus-1-dev build-essential pkg-config file
# Second screen (GStreamer, Linux only): headers to build, plugins to run
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-good
(cd apps/desktop && npm install)   # once
scripts/desktop-dev.sh             # = npm run tauri dev, with a clean environment
```

Keyboard shortcuts need the uinput udev rule of the
[Linux guide](install-linux.md#2-allow-keyboard-shortcuts-uinput).

Use `scripts/desktop-dev.sh` rather than `npm run tauri dev` from a terminal
of an app installed as a **snap** (e.g. VS Code): snap terminals leak GTK
variables that make WebKitGTK abort at startup. The script removes them.
From such a terminal, the play/pause state of snap players (Firefox) also
stays hidden (AppArmor refuses their MPRIS calls); the buttons still work.

The app serves tablets on two ports:

- **USB**: `127.0.0.1:47321`, always on, loopback only. The tablet reaches it
  through `adb reverse tcp:47321 tcp:47321` (run by the app on every plug) and
  needs no pairing.
- **Wi-Fi**: `0.0.0.0:47322`, off at first launch, WebSocket over TLS, PIN
  pairing then key pinning, announced over mDNS (`_padboard._tcp`).

| Variable | Values | Default |
|---|---|---|
| `PADBOARD_BIND` | `lan` (force Wi-Fi on), `loopback` (force it off) | the setting in the app |
| `PADBOARD_PORT` | USB port | `47321` |
| `PADBOARD_WIFI_PORT` | Wi-Fi port | `47322` |
| `PADBOARD_LOCALE` | `en`, `fr` | the system language |
| `PADBOARD_LOG` | `tracing` filter, e.g. `debug` | `info` (debug for Padboard) |

The configuration (profiles, PC identity, TLS key, paired devices,
preferences) is in `~/.config/io.github.hugoambiehl.padboard/` (Windows:
`%APPDATA%\io.github.hugoambiehl.padboard\`). Deleting `tls-key.pem` or
`identity.json` makes every tablet pair again.

### Preview the UI in a browser

`cd apps/desktop && npm run dev`, then open `http://localhost:1420/?mock`
(`&view=connections` for the connections view, `&lang=en` for English): the
frontend runs against a fake backend (`src/dev/mock.ts`, development builds
only). Handy for layout work; everything else needs the real app.

### Test without the tablet

`fake-tablet` speaks the protocol like the Android app (mDNS discovery, TLS
with the PC's key pinned, PIN pairing, token, USB without pairing):

```sh
cargo run -p padboard-desktop --example fake-tablet -- discover          # PCs announced over mDNS
cargo run -p padboard-desktop --example fake-tablet -- wifi 127.0.0.1    # pair, then reconnect with the token
cargo run -p padboard-desktop --example fake-tablet -- usb               # loopback port, no pairing
cargo run -p padboard-desktop --example fake-tablet -- forget            # drop its pairing
```

`--id NAME` simulates another tablet. Once connected, type a button id (e.g.
`terminal`) and Enter to press it.

`fake-obs` is a small obs-websocket v5 server (no password) with a scene
list, for the OBS settings of the connections view:

```sh
cargo run -p padboard-desktop --example fake-obs -- --port 4455 Jeu Pause
```

Then type `create <scene>`, `rename <old> <new>` or `remove <scene>` to send
the matching OBS event.

Actions alone, without the app:

```sh
cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT   # after a 3 s countdown
cargo run -p padboard-actions --example action -- '<action JSON>'
cargo run -p padboard-actions --example state -- --watch                    # observed audio / media state
```

### Test mode

`PADBOARD_TEST_DIR=<absolute dir>` starts the app on a separate
configuration in that directory, never yours: actions are recorded to
`<dir>/actions.jsonl` and never run, USB devices (adb) and Wi-Fi are left
alone, logs go to `<dir>/app.log`, the window title says "mode test", there
is no tray icon (closing the window quits; `PADBOARD_TEST_TRAY=1` adds one
named "Padboard (test)", for `e2e/tray`), and
button states come from `<dir>/state.json`, e.g.
`{"audio": {"outputPercent": 50, "outputMuted": false, "inputMuted": true}, "mediaPlaying": false}`.

Add `PADBOARD_TEST_OBS=1` to reach OBS for real (with the test
configuration's `obs.json`, e.g. pointing to `fake-obs`) instead of reading
its state from `state.json`.

Add `PADBOARD_TEST_SYSTEM=1` to run actions and observe states for real (the
configuration stays in the test directory; Wi-Fi can be forced on with
`PADBOARD_BIND=lan`). Helpers for such tests:

```sh
cargo run -p padboard-actions --example audio-snapshot -- save /tmp/audio.json     # before
cargo run -p padboard-actions --example audio-snapshot -- restore /tmp/audio.json  # after, exact
cargo run -p padboard-actions --example fake-player   # an MPRIS player for media buttons
```

### Second screen (in development, #20)

The *Second écran* action is listed only where the PC has a picture
source, for now only on request (Linux, GStreamer):
`PADBOARD_SCREEN_SOURCE=screen` (a virtual monitor through the
ScreenCast portal: GNOME asks the first time, then remembers it with the
token in `screen-cast-token` in the config dir) or
`PADBOARD_SCREEN_SOURCE=pattern` (a test pattern, no portal, any mode). A tablet whose app announces the capability then gets the
stream; the PC shows "Second écran : <tablet>" with *Arrêter* in the
header and in the tray menu.

Measurements (ignored tests, `--ignored --nocapture`):
`screen::gst::tests::throughput` (the pattern, frames per second with
acks after `PADBOARD_BENCH_ACK_MS`, default 20) and
`screen::gst::tests::screen_cost` (the real screen: asks to share it,
then gives 45 s to put something moving on the new, invisible monitor,
e.g. a window pushed past the right edge and maximized with Super+Up;
then frames per second and the CPU of Padboard and gnome-shell per
encoder, `PADBOARD_BENCH_SECS` each).

### End-to-end tests (real window, local only)

`scripts/desktop-e2e.sh` drives the real app window with
[tauri-driver](https://v2.tauri.app/develop/tests/webdriver/) (WebKitWebDriver)
in test mode and runs `apps/desktop/e2e/*.e2e.ts`. They are **not run in CI**.

```sh
sudo apt install webkitgtk-webdriver xvfb  # once (xvfb: optional; Ubuntu 24.04: webkit2gtk-driver)
cargo install tauri-driver --locked        # once
scripts/desktop-e2e.sh                     # build (frontend embedded), then every test
scripts/desktop-e2e.sh --no-build profiles # reuse the build, only matching files
PADBOARD_E2E_HEADLESS=1 scripts/desktop-e2e.sh   # on a virtual display
```

Screenshots of the key steps (and of any failure) go to
`target/e2e/screenshots/`; review them after UI changes.

## Desktop app (Windows)

Git, Node.js 24, Rust (`rustup`, MSVC toolchain) and Visual Studio Build
Tools 2022 with *Desktop development with C++*:

```powershell
cd apps\desktop
npm ci
npm run tauri dev                 # development
npx tauri build --bundles nsis    # installer in ..\..\target\release\bundle\nsis\
```

From Linux, `cargo clippy -p padboard-actions --target x86_64-pc-windows-msvc`
checks the Windows actions; the CI builds and tests everything on Windows.
The manual test plan of a Windows build is [windows-test-plan.md](windows-test-plan.md).

## Tablet app

Requirements: JDK 17+, the Android SDK (`ANDROID_HOME`), a tablet with USB
debugging on.

```sh
cd apps/android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity
adb logcat --pid="$(adb shell pidof io.github.hugoambiehl.padboard)"   # logs
```

A debug build and a release build are signed with different keys: switching
between them needs `adb uninstall` (pairing lost). Release builds, signing
and kiosk mode: [release-signing.md](release-signing.md),
[install-tablet.md](install-tablet.md#kiosk-mode).

UI tests (`app/src/androidTest`, e.g. `KioskCornerTest`: nothing on any
screen in the corner that leaves kiosk mode) run on a throwaway emulator
at the tablet's size, created and deleted by the script (your own AVDs
are not touched; see the script for the API 37 image's quirks):

```sh
scripts/tablet-ui-tests.sh
PADBOARD_TEST_IMAGE="system-images;android-32;google_apis;x86_64" scripts/tablet-ui-tests.sh
```

The second line runs them on Android 12L, the tablet's version: the only
emulator where `KioskSingleInstanceTest` catches two screens of the app in
kiosk mode (API 24 guards #24; the API 37 emulator skips it).
`PADBOARD_TEST_GPU`, `PADBOARD_TEST_RAM` and `PADBOARD_TEST_ARGS` (more
`am instrument` arguments) change the run.

The older system images are not kept installed (3–4 GB each). Install the
one a run needs, then remove it afterwards:

```sh
sdkmanager="$HOME/Android/Sdk/cmdline-tools/latest/bin/sdkmanager"
"$sdkmanager" "system-images;android-32;google_apis;x86_64"   # Android 12L, the tablet's
"$sdkmanager" "system-images;android-24;default;x86_64"       # Android 7.0, minSdk (#21, #24)
PADBOARD_TEST_IMAGE="system-images;android-24;default;x86_64" scripts/tablet-ui-tests.sh
"$sdkmanager" --uninstall "system-images;android-24;default;x86_64"
```

Three more androidTests are skipped unless given their arguments, since they
need a PC or change the device. The first two were run on an Android 7.0
(API 24) emulator for #21.

- `LocalNetworkTest` (Android 17 only, #31): no system device picker at
  launch, and kiosk mode grants itself the local network permission. The
  API 37 emulator aborts its system_server now and then when a task
  starts, so it runs alone, on a fresh emulator (it needs the permission
  not granted yet):

  ```sh
  PADBOARD_TEST_GPU=swiftshader_indirect PADBOARD_TEST_RAM=4096 \
  PADBOARD_TEST_ARGS="-e class io.github.hugoambiehl.padboard.connection.LocalNetworkTest -e padboardLocalNetwork true" \
    scripts/tablet-ui-tests.sh
  ```

- `PinnedTlsTest`: a real TLS handshake with a Padboard desktop, pinned
  like the app does: accepted with the PC's key, refused with another
  one. Start the desktop in test mode with Wi-Fi on
  (`PADBOARD_TEST_DIR=… PADBOARD_TEST_SYSTEM=1 PADBOARD_BIND=lan`), then,
  from an emulator (`10.0.2.2` is the host):

  ```sh
  adb shell am instrument -w -e class io.github.hugoambiehl.padboard.connection.PinnedTlsTest \
    -e padboardHost 10.0.2.2 -e padboardPort 47322 \
    io.github.hugoambiehl.padboard.test/androidx.test.runner.AndroidJUnitRunner
  adb logcat -d -s PinnedTlsTest:I   # the TLS version and cipher suite
  ```

- `SilentUpdateTest`: the silent update of a kiosk tablet (device owner,
  `AndroidUpdater`). The app must be the device owner
  (`adb shell dpm set-device-owner io.github.hugoambiehl.padboard/.kiosk.KioskAdmin`
  on a throwaway emulator), with a newer build of it in its cache folder:

  ```sh
  ./gradlew assembleDebug -Ppadboard.testVersionCode=30001   # then copy the APK aside
  adb push update.apk /data/local/tmp/
  adb shell "cat /data/local/tmp/update.apk | run-as io.github.hugoambiehl.padboard sh -c 'cat > cache/update-test.apk'"
  adb shell am instrument -w -e class io.github.hugoambiehl.padboard.update.SilentUpdateTest \
    -e padboardUpdateApk update-test.apk \
    io.github.hugoambiehl.padboard.test/androidx.test.runner.AndroidJUnitRunner
  ```

  When the update succeeds, Android replaces the app and ends the test's
  process, so the instrumentation reports "Process crashed". Check the
  result afterwards: `dumpsys package` shows the new versionCode, and the
  app is back in front, still locked (`mLockTaskModeState=LOCKED`).

Measure the tablet's performance on **release** builds only
(`dumpsys gfxinfo <package> framestats`): debug builds are several times
slower.

## Checks

The CI (`.github/workflows/ci.yml`) runs on every push and pull request:

| Job | Checks |
| --- | --- |
| Rust + desktop frontend (Linux) | ESLint, Prettier, Vitest, type-check, `vue-tsc` + Vite build, `cargo fmt`, `cargo clippy -D warnings`, `cargo test` |
| Generated protocol types | `scripts/gen-types.sh --check` |
| Android | ktlint, unit tests, Android Lint (with the Licensee license check), debug APK |
| Secret scan | gitleaks on the whole history (`.gitleaks.toml`) |
| Licenses and advisories | `cargo deny check` (licenses, sources, RustSec), `scripts/licenses.sh` |
| Rust (Windows) | `cargo clippy -D warnings`, `cargo test` (on `main` and pull requests) |

The same checks locally:

```sh
(cd apps/desktop && npm run lint && npm run format:check && npm test && npm run typecheck && npm run build)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/gen-types.sh --check
cargo deny check && scripts/licenses.sh
(cd apps/android && ./gradlew ktlintCheck testDebugUnitTest lintDebug assembleDebug)
```

`cargo deny` and `scripts/licenses.sh` need
`cargo install cargo-deny --version 0.20.2 --locked` and
`cargo install cargo-about --version 0.9.3 --features cli --locked`.

## Third-party licenses

Every dependency must be under a GPL-compatible license; the allow-lists are
enforced at build time: `deny.toml` and `about.toml` (Rust),
`NPM_ALLOWED` in `scripts/licenses.mjs` (npm), `licensee { allow(...) }` in
`apps/android/app/build.gradle.kts` (Android). The apps show the full list
with the license texts (*About* → *Licenses*): `scripts/licenses.sh` builds
it for the desktop (release builds do; development builds show an empty
list until you run it once).

## Icons

- **App icon**: `assets/icon/padboard.svg` is the single source;
  `node scripts/icons.mjs` regenerates every file from it (desktop PNGs,
  `.ico`, tray, favicon, Android drawables). Never edit those by hand.
- **Lucide upgrades**: buttons use [Lucide](https://lucide.dev) icons, sent
  to the tablet as SVG paths that the desktop validates. After upgrading
  `lucide`, export the icons with `node scripts/export-icons.mts /tmp/lucide.json` (from
  `apps/desktop`), then run
  `PADBOARD_LUCIDE_JSON=/tmp/lucide.json cargo test -p padboard-desktop -- --ignored lucide`
  so every icon still passes the validator.

## Development version and the tablet

Between two releases, `main` carries a **development version**
(`X.Y.Z-dev`, Android versionCode `MAJOR·10⁶ + MINOR·10⁴ + PATCH·100`,
between the previous release and the first release candidate). `release.sh`
moves `main` to the next one right after tagging a release, and the CI
(`scripts/check-version.sh`) refuses `main` with a released version, or
files that disagree; `scripts/dev-version.sh X.Y.Z-dev` sets it by hand.

A development build has no published APK: it never offers an update to a
tablet. After a protocol change, a tablet running the published app cannot
connect to it: the USB card and the *Connected tablets* card say so. Then
either start the **installed** Padboard (quit the development one first:
same ports), or run the development build in test mode
(`PADBOARD_TEST_DIR`, see below) with a development APK on the tablet. A
development APK is signed with the debug key: installing it over the
published app needs `adb uninstall` first (leave kiosk mode first; the
Wi-Fi pairing is lost), and going back to the published app as well.
`scripts/desktop-dev.sh` warns when it runs on the real configuration.

## Testing updates without publishing

A real update (desktop or tablet) is tested against **local releases**,
never a published one:

```sh
# Test APKs with other versions (debug key; release builds keep their version)
(cd apps/android && ./gradlew assembleDebug -Ppadboard.testVersionCode=20002 -Ppadboard.testVersionName=0.2.0-rc.2)
# A local release, signed with a test key created in target/test-releases
scripts/test-releases.sh add 0.2.0-rc.2 --apk apps/android/app/build/outputs/apk/debug/app-debug.apk
scripts/test-releases.sh serve       # http://127.0.0.1:8765
scripts/test-releases.sh env         # the variables below
# A desktop build that reads them (test mode, real adb and Wi-Fi)
cargo build -p padboard-desktop --features test-updates
PADBOARD_TEST_DIR=… PADBOARD_TEST_SYSTEM=1 PADBOARD_BIND=lan \
  PADBOARD_TEST_RELEASES=http://127.0.0.1:8765 \
  PADBOARD_TEST_UPDATE_KEY=target/test-releases/test-update.key.pub \
  PADBOARD_TEST_APP_VERSION=0.2.0-rc.2 target/debug/padboard-desktop
```

Only builds with the `test-updates` feature read these variables
(`updates/source.rs`); a test proves shipped builds ignore them and the
release workflow refuses a binary containing them. On Windows, the
installer update over `http://` also needs
`npx tauri build --features test-updates --config src-tauri/tauri.test-updates.conf.json`.

## Releases

See [releasing.md](releasing.md).
