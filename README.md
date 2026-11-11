# Padboard

DIY Stream Deck: a desktop app (Windows / Linux) that runs actions on your PC,
and an Android app that turns a dedicated tablet into a touch control surface.
Wi-Fi or USB, no cloud, free and open-source tools only.

> 🚧 Early development: connection (USB / Wi-Fi) and the editor work; the
> only action so far is a keyboard shortcut (more actions come in phase 4).

## Repository layout

| Path | Content |
|---|---|
| `crates/protocol` | Wire protocol (source of truth, TS/Kotlin types generated with typeshare) |
| `crates/actions` | Action execution (`ActionExecutor`, Linux and Windows implementations) |
| `apps/desktop` | Desktop app (Tauri 2, Vue 3) |
| `apps/android` | Tablet app (Kotlin, Jetpack Compose) |
| `docs/design` | Reference mockups |
| `scripts/gen-types.sh` | Regenerates the TS and Kotlin protocol types |

## Using it

- **Editor** (main window): profiles, pages and a replica of the tablet grid.
  Drag an action from the library onto a cell (or click it to fill the
  selected cell), select a button to change its label, icon (Lucide), color
  and keyboard shortcut. Arrows move the selection, Alt + arrows move the
  button, Delete removes it, Ctrl+Z / Ctrl+Shift+Z undo and redo. Changes are
  saved and pushed to the tablets as you type.
- **Profiles**: the profile menu chooses the profile you edit; **Afficher sur
  la tablette** chooses the one the tablets show (marked in the menu).
- **Tablet**: swipe to change page; the page stays in front of you through
  edits and reconnections.
- **Actions**: keyboard shortcut, media (play/pause, next, previous, stop),
  PC volume, microphone on/off, open a URL / file / folder / program, lock the
  session, page and folder navigation, multi actions (steps with pauses) and
  OBS Studio (scene, record, live, source mute). Microphone, volume, play/pause
  and OBS buttons show their state on the tablet; a look "while active" can be
  customised.
- **OBS Studio**: in OBS, *Tools → WebSocket Server Settings → Enable*, then in
  Padboard's connections view turn on *Piloter OBS* with the port and password.
  OBS buttons are dimmed while OBS is closed and come back by themselves.
- **Connexions** (the tablet pill in the header): server, connected and paired
  tablets, USB devices, OBS, recent presses.

## Development setup (Linux)

### Run the desktop app

```sh
(cd apps/desktop && npm install)   # once
scripts/desktop-dev.sh             # = npm run tauri dev, with a clean environment
```

The app serves tablets on two ports:

- **USB**: `127.0.0.1:47321`, always on, loopback only. The tablet reaches it
  through `adb reverse tcp:47321 tcp:47321` and needs no pairing (the cable is
  the proof).
- **Wi-Fi**: `0.0.0.0:47322`, **off at first launch**, WebSocket over TLS. It is
  turned on, and stays on across restarts, from the status screen toggle or the
  **Activer le Wi-Fi** button of the pairing dialog. A tablet
  must pair once with a PIN shown on the PC; it then pins the PC's public key.
  When on, the PC is announced over mDNS (`_padboard._tcp`). Your firewall must
  allow TCP 47322 and UDP 5353 (mDNS); Windows asks the first time.

| Variable | Values | Default |
|---|---|---|
| `PADBOARD_BIND` | `lan` (force Wi-Fi on), `loopback` (force it off) | the setting in the app |
| `PADBOARD_PORT` | USB port | `47321` |
| `PADBOARD_WIFI_PORT` | Wi-Fi port | `47322` |
| `PADBOARD_LOG` | `tracing` filter, e.g. `debug` | `info` (debug for Padboard) |

The configuration (profiles, PC identity, Wi-Fi TLS key, paired devices,
settings) lives in
`~/.config/io.github.hugoambiehl.padboard/` (Linux) or
`%APPDATA%\io.github.hugoambiehl.padboard\` (Windows). Deleting
`tls-key.pem` or `identity.json` makes every tablet pair again.

**Shared PCs:** any program running on the PC can connect to the USB port, like
the tablet does through `adb reverse`, without pairing. On a PC used by several
user accounts at the same time, another account could therefore send keyboard
shortcuts into your session while Padboard runs.

From a snap terminal (VS Code), the play/pause state of snap players such as
Firefox stays hidden (AppArmor refuses their MPRIS calls to snap processes);
the buttons still work. The app started from the desktop has no such limit.

Use `scripts/desktop-dev.sh` rather than `npm run tauri dev` when your terminal
belongs to an app installed as a **snap** (e.g. VS Code): snap terminals leak GTK
variables that make WebKitGTK abort at startup (`Settings schema … does not
contain a key named 'antialiasing'`). The script removes them.

### Preview the UI in a browser

`cd apps/desktop && npm run dev`, then open `http://localhost:1420/?mock`
(`&view=connections` for the connections view): the frontend runs against a fake
backend (`src/dev/mock.ts`, development builds only). Handy for layout work and
headless screenshots; everything else needs the real app.

### Test without the tablet

`fake-tablet` speaks the protocol like the Android app (mDNS discovery, TLS with
the PC's key pinned, PIN pairing, token, USB without pairing):

```sh
cargo run -p padboard-desktop --example fake-tablet -- discover          # PCs announced over mDNS
cargo run -p padboard-desktop --example fake-tablet -- wifi 127.0.0.1    # pair, then reconnect with the token
cargo run -p padboard-desktop --example fake-tablet -- usb               # loopback port, no pairing
cargo run -p padboard-desktop --example fake-tablet -- forget            # drop its pairing
```

`--id NAME` simulates another tablet. Once connected, type a button id (e.g.
`terminal`) and Enter to press it.

### End-to-end tests (real window, local only)

`scripts/desktop-e2e.sh` drives the real app window with
[tauri-driver](https://v2.tauri.app/develop/tests/webdriver/) (WebKitWebDriver
on Linux) and runs the tests in `apps/desktop/e2e/` (editor scenario, profiles,
persistence, connections view with the fake tablet). They are **not run in CI**.

One-time setup:

```sh
sudo apt install webkit2gtk-driver      # WebKitWebDriver
cargo install tauri-driver --locked
sudo apt install xvfb                   # optional, for PADBOARD_E2E_HEADLESS=1
```

```sh
scripts/desktop-e2e.sh                     # build (frontend embedded), then all tests
scripts/desktop-e2e.sh --no-build profiles # reuse the build, only matching files
PADBOARD_E2E_HEADLESS=1 scripts/desktop-e2e.sh   # on a virtual display (xvfb)
```

Without `PADBOARD_E2E_HEADLESS`, the windows open on your screen for a few
seconds. Screenshots of the key steps (and of any failure) go to
`target/e2e/screenshots/`. The build uses its own target directory
(`target/e2e`) so it does not invalidate the `tauri dev` build.

The app runs in **test mode** (`PADBOARD_TEST_DIR=<absolute dir>`): the
configuration lives in that directory instead of yours, actions are recorded
to `<dir>/actions.jsonl` and never injected, USB devices (adb) and Wi-Fi are
left alone, logs go to `<dir>/app.log`, and the window title says "mode test".
Button states are simulated from `<dir>/state.json` instead of the real
microphone, sound server and players, e.g.
`{"audio": {"outputPercent": 50, "outputMuted": false, "inputMuted": true}, "mediaPlaying": false}`
(`"audio": "unavailable"` for a missing sound server).
The tests create a temporary directory per run (`PADBOARD_E2E_KEEP=1` keeps it).

To try the real thing without touching your configuration, add
`PADBOARD_TEST_SYSTEM=1` to a test-mode launch: the configuration stays in
the test directory, but actions really run and states are really observed.
Helpers for such tests (Linux):

```sh
cargo run -p padboard-actions --example audio-snapshot -- save /tmp/audio.json     # before
cargo run -p padboard-actions --example audio-snapshot -- restore /tmp/audio.json  # after, exact
cargo run -p padboard-actions --example fake-player   # an MPRIS player for media buttons
```

### Run the tablet app

Requirements: JDK 17+ and the Android SDK (`ANDROID_HOME`), `adb`, USB debugging
enabled on the tablet.

```sh
cd apps/android
./gradlew :app:assembleDebug                      # builds app/build/outputs/apk/debug/app-debug.apk
adb devices                                       # the tablet must be listed as "device" (accept the prompt on it)
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity
```

How the tablet connects:

- **USB** (tried first): with the desktop app running, plugging the tablet is
  enough. The desktop runs `adb reverse` by itself and the tablet connects to
  `127.0.0.1:47321`, without pairing.
- **Wi-Fi**: enable Wi-Fi in the desktop app, click **Associer un appareil**, pick
  the PC in the list on the tablet (found over mDNS, or type its address) and
  type the 6-digit code. Both screens show the PC's key fingerprint: they must
  match. The tablet then reconnects by itself.
- Plugging the cable while on Wi-Fi switches to USB without a gap; unplugging it
  falls back to Wi-Fi. Revoking the tablet on the PC makes it pair again.

Logs: `adb logcat --pid="$(adb shell pidof io.github.hugoambiehl.padboard)"`.

Checks: `./gradlew :app:testDebugUnitTest` (shared protocol fixtures and the
connection manager against a fake PC with both transports) and
`./gradlew :app:ktlintCheck` (`ktlintFormat` to fix).

### Linux: uinput access

On Linux, Padboard sends keyboard shortcuts through a **uinput virtual keyboard**.
It works on both X11 and Wayland, for every application, but the desktop app
needs write access to `/dev/uinput`.

Padboard does **not** require adding your user to the `input` group (that would
give read access to every input device, keyboard included). Instead, a udev rule
tags `/dev/uinput` with `uaccess`: systemd-logind then grants access only to the
user of the **active local session**, and revokes it when the session ends or
another user switches in.

Install the rule (once per machine):

```sh
echo 'KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

- The file name must sort **before** `73-seat-late.rules`, which is where
  `uaccess` tags are turned into permissions — hence the `70-` prefix.
- `static_node=uinput` makes `/dev/uinput` exist at boot even before the
  `uinput` kernel module is loaded (opening it loads the module).
- No reboot or logout is needed.

Check the result — your user must appear with `rw-`:

```sh
getfacl -p /dev/uinput
# user:<you>:rw-
```

Then test injection without the app (sends the shortcut after a 3-second countdown):

```sh
cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT
```

Keys are W3C [`KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/) names,
i.e. **physical positions**: `KeyQ` is the key labelled "A" on an AZERTY keyboard.

Without the rule, the desktop app keeps running but keyboard shortcuts fail with
an explicit error pointing to this section. To remove the rule:

```sh
sudo rm /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

> Security note: with this rule, any program running in your session can create
> a virtual keyboard and inject keystrokes (it cannot read your real keyboard).
> This is the same trust level as an X11 session.

## Continuous integration

GitHub Actions (`.github/workflows/ci.yml`) runs on every push:

| Job | Checks |
| --- | --- |
| Rust + desktop frontend (Linux) | ESLint, Prettier, Vitest, type-check of the e2e tests (not run), `vue-tsc` + Vite build, `cargo fmt`, `cargo clippy -D warnings`, `cargo test` |
| Generated protocol types | `scripts/gen-types.sh --check` |
| Android | ktlint, unit tests, Android Lint, debug APK |
| Rust (Windows) | `cargo clippy -D warnings`, `cargo test` — only on pushes to `main`, pull requests and manual runs |

To run the same checks locally before pushing:

```sh
(cd apps/desktop && npm run lint && npm run format:check && npm test && npm run typecheck && npm run build)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/gen-types.sh --check
(cd apps/android && ./gradlew ktlintCheck testDebugUnitTest lintDebug assembleDebug)
```

Run the whole workflow by hand (Windows job included): `gh workflow run CI --ref <branch>`.
