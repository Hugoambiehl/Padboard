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
alone, logs go to `<dir>/app.log`, the window title says "mode test", and
button states come from `<dir>/state.json`, e.g.
`{"audio": {"outputPercent": 50, "outputMuted": false, "inputMuted": true}, "mediaPlaying": false}`.

Add `PADBOARD_TEST_SYSTEM=1` to run actions and observe states for real (the
configuration stays in the test directory; Wi-Fi can be forced on with
`PADBOARD_BIND=lan`). Helpers for such tests:

```sh
cargo run -p padboard-actions --example audio-snapshot -- save /tmp/audio.json     # before
cargo run -p padboard-actions --example audio-snapshot -- restore /tmp/audio.json  # after, exact
cargo run -p padboard-actions --example fake-player   # an MPRIS player for media buttons
```

### End-to-end tests (real window, local only)

`scripts/desktop-e2e.sh` drives the real app window with
[tauri-driver](https://v2.tauri.app/develop/tests/webdriver/) (WebKitWebDriver)
in test mode and runs `apps/desktop/e2e/*.e2e.ts`. They are **not run in CI**.

```sh
sudo apt install webkit2gtk-driver xvfb    # once (xvfb: optional)
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
and kiosk mode: [android-release.md](android-release.md),
[install-tablet.md](install-tablet.md#kiosk-mode).

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

## Releases

See [releasing.md](releasing.md).
