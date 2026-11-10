# Padboard

DIY Stream Deck: a desktop app (Windows / Linux) that runs actions on your PC,
and an Android app that turns a dedicated tablet into a touch control surface.
Wi-Fi or USB, no cloud, free and open-source tools only.

> 🚧 Early development (phase 1 — foundations). Not usable yet.

## Repository layout

| Path | Content |
|---|---|
| `crates/protocol` | Wire protocol (source of truth, TS/Kotlin types generated with typeshare) |
| `crates/actions` | Action execution (`ActionExecutor`, Linux and Windows implementations) |
| `apps/desktop` | Desktop app (Tauri 2, Vue 3) |
| `apps/android` | Tablet app (Kotlin, Jetpack Compose) |
| `docs/design` | Reference mockups |
| `scripts/gen-types.sh` | Regenerates the TS and Kotlin protocol types |

## Development setup (Linux)

### Run the desktop app

```sh
(cd apps/desktop && npm install)   # once
scripts/desktop-dev.sh             # = npm run tauri dev, with a clean environment
```

The WebSocket server listens on `127.0.0.1:47321` by default: enough for a tablet
plugged over USB, after `adb reverse tcp:47321 tcp:47321`. Pairing does not exist
yet, so listening on the local network (Wi-Fi) is an explicit opt-in, and the app
shows a warning while it is enabled:

```sh
PADBOARD_BIND=lan scripts/desktop-dev.sh
```

| Variable | Values | Default |
|---|---|---|
| `PADBOARD_BIND` | `loopback`, `lan` | `loopback` |
| `PADBOARD_PORT` | TCP port | `47321` |
| `PADBOARD_LOG` | `tracing` filter, e.g. `debug` | `info` (debug for Padboard) |

Use `scripts/desktop-dev.sh` rather than `npm run tauri dev` when your terminal
belongs to an app installed as a **snap** (e.g. VS Code): snap terminals leak GTK
variables that make WebKitGTK abort at startup (`Settings schema … does not
contain a key named 'antialiasing'`). The script removes them.

### Run the tablet app (USB)

Requirements: JDK 17+ and the Android SDK (`ANDROID_HOME`), `adb` in the PATH,
USB debugging enabled on the tablet. Phase 1 only connects by manual address;
the desktop will run `adb reverse` by itself in phase 2.

```sh
cd apps/android
./gradlew :app:assembleDebug                      # builds app/build/outputs/apk/debug/app-debug.apk
adb devices                                       # the tablet must be listed as "device" (accept the prompt on it)
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb reverse tcp:47321 tcp:47321                   # tablet 127.0.0.1:47321 -> PC 127.0.0.1:47321
adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity
```

On the tablet, tap **Utiliser l’USB** (or type `127.0.0.1`, port `47321`). The
address is remembered and the app reconnects to it at startup.

`adb reverse` is lost when the cable is unplugged or the adb server restarts:
run it again. Logs: `adb logcat --pid="$(adb shell pidof io.github.hugoambiehl.padboard)"`.

Checks: `./gradlew :app:testDebugUnitTest` (includes the shared protocol fixtures)
and `./gradlew :app:ktlintCheck` (`ktlintFormat` to fix).

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
| Rust + desktop frontend (Linux) | ESLint, Prettier, `vue-tsc` + Vite build, `cargo fmt`, `cargo clippy -D warnings`, `cargo test` |
| Generated protocol types | `scripts/gen-types.sh --check` |
| Android | ktlint, unit tests, Android Lint, debug APK |
| Rust (Windows) | `cargo clippy -D warnings`, `cargo test` — only on pushes to `main`, pull requests and manual runs |

To run the same checks locally before pushing:

```sh
(cd apps/desktop && npm run lint && npm run format:check && npm run build)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/gen-types.sh --check
(cd apps/android && ./gradlew ktlintCheck testDebugUnitTest lintDebug assembleDebug)
```

Run the whole workflow by hand (Windows job included): `gh workflow run CI --ref <branch>`.
