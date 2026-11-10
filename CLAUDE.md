# Padboard

DIY Stream Deck: a **desktop app** (Tauri 2) that executes actions on the PC, and
an **Android app** (Kotlin + Compose) used as a touch control surface on a dedicated
tablet. Private repo for now, will become open source.

**Absolute rule: only free and open-source tools, libraries and APIs.** No paid API,
nothing that requires approval, no Google Play Services / Firebase / ML Kit.

## Targets

- **Desktop**: Windows 10/11 (main target), Linux (dev + local tests, with real
  Linux implementations of actions, not stubs). Dev machine: Ubuntu 24.04, GNOME
  **Wayland**. Windows installers are built by GitHub Actions on a Windows runner.
- **Tablet**: Samsung Galaxy Tab A 10.1 (SM-T580), 2 GB RAM, 1920×1200
  (≈ 1280×800 dp), landscape. LineageOS 19.1 = Android 12L (API 32), **no Play
  Services, no vibration motor**. `minSdk 26`, `targetSdk` = latest stable.

## Stack

- **Desktop**: Tauri 2. Rust backend; frontend Vue 3 + TypeScript + Tailwind,
  Motion for Vue (GSAP if needed), Lucide icons. Tray icon + autostart (Tauri plugins).
- **Android**: Kotlin + Jetpack Compose, OkHttp WebSocket, kotlinx.serialization,
  NsdManager, DataStore. Dependencies declared in `gradle/libs.versions.toml` only.
- **Protocol**: JSON over WebSocket, envelope `{ type, payload }`, versioned
  (version checked at handshake). Source of truth: `crates/protocol`.
- Monorepo, Cargo workspace at the root.

## Roles

- **Desktop = the brain**: stores all config (profiles, pages, folders, buttons),
  runs actions, exposes the WebSocket server, provides the editor. Sends icons
  (Lucide SVG or user images converted to PNG).
- **Tablet = dumb client**: renders the grid it receives, sends presses/releases,
  receives state updates, caches icons. **No business logic on the tablet.**

## Connection (no Bluetooth)

Same WebSocket protocol over two transports:
- **USB** (tried first): desktop detects the tablet via `adb` and runs
  `adb reverse tcp:PORT tcp:PORT`; tablet connects to `127.0.0.1:PORT`. No PIN
  (the cable is the proof). `adb` from PATH, otherwise the app offers to download
  Platform Tools or pick its location.
- **Wi-Fi**: mDNS (`mdns-sd`) ↔ NsdManager. 6-digit PIN shown on desktop (expires
  after 2 min) → persistent token. Paired devices list is revocable. Android needs
  a `network_security_config` allowing cleartext `ws://` on the LAN.
- Auto-reconnect with backoff, transparent USB ↔ Wi-Fi switch, ping/pong heartbeat
  (latency shown in UI).

## Repo layout

```
crates/protocol        # protocol types (#[typeshare]) — SOURCE OF TRUTH
crates/actions         # ActionExecutor trait + Windows / Linux impls (cfg target_os)
apps/desktop           # Tauri 2 (src-tauri = Rust, src = Vue/TS)
apps/android           # Gradle project (created by Android Studio — never recreate it)
docs/design            # reference mockups (Main, Pairing, Editor .dc.html)
scripts/gen-types.sh   # typeshare -> TS + Kotlin types
Cargo.toml             # workspace
```

## ⚠️ Protocol rule

**The protocol is modified ONLY in `crates/protocol`.** Then regenerate the TS and
Kotlin types with `scripts/gen-types.sh`. Never hand-edit generated files (they
carry a "generated" header). Bump the protocol version on breaking changes and
add/adjust round-trip serialization tests.

## Actions

`ActionExecutor` trait in `crates/actions`, implementation picked at compile time:
- **Windows** (`windows` crate): SendInput, IAudioEndpointVolume,
  GlobalSystemMediaTransportControls…
- **Linux**: keyboard injection, MPRIS over D-Bus, PipeWire/PulseAudio, `xdg-open`,
  `loginctl lock-session`.
- An action with no Linux equivalent must **log clearly and return an error**,
  never panic.
- Cross-platform: OBS WebSocket v5. Discord only via its global shortcuts (no RPC).

## Design

Reference mockups in `docs/design/*.dc.html` (HTML mockups, not code to reuse):
reproduce layout, colors, sizes and texts faithfully.

- Background `#0E0F12`; surfaces `#121317` / `#17181D`; active surface `#1E2029`
- Borders `#22242B` / `#24262E`; text `#E8EAF0`; secondary `#8A90A0` / `#A3ABBD`
- Accent `#8B7CFF`
- Category colors (icon + background at ~16 % opacity): violet `#8B7CFF`,
  blue `#5AA9FF`, teal `#3DD6B5`, orange `#FF9F5A`, pink `#FF6FAE`,
  red `#FF5C5C`, grey `#A3ABBD`
- Fonts: Space Grotesk (titles), Inter (text) — **bundled, no CDN**
- Radii: tablet tiles 22 dp, small elements 10–16 dp. Dark theme only.

**Tablet animation rule (strict)**: only `transform` and `alpha` through
`graphicsLayer`. Never animate layout, never animate blur. Target 60 fps on the
SM-T580, always measure in release builds.

## Durable decisions

- **Linux keyboard injection: uinput** (`evdev` crate), works on X11 and Wayland.
  The udev rule granting access to `/dev/uinput` is documented in the README.
- **Default port 47321.** Server binds **`127.0.0.1` by default** (enough for USB
  via `adb reverse`). LAN bind (`0.0.0.0`) only on explicit opt-in
  (`PADBOARD_BIND` env var or setting), with a visible warning in the frontend
  as long as pairing does not exist.
- **Keys are stored by physical position**: W3C `KeyboardEvent.code` names
  (`KeyA`, `ControlLeft`…), mapped to evdev keycodes / Windows scancodes.
- **Generated types are committed**: `apps/desktop/src/generated/` (TS) and the
  Android package `io.github.hugoambiehl.padboard.protocol` (Kotlin). CI checks
  they are up to date.
- **Type generation**: `scripts/gen-types.sh` (`--check` in CI), config in
  `typeshare.toml`, typeshare-cli version pinned in the script. typeshare has no
  option to emit string unions, so it emits TS `enum`s: `erasableSyntaxOnly` is
  disabled in the desktop tsconfig for that reason. No custom post-processing of
  typeshare output.
- **Shared JSON fixtures** in `crates/protocol/fixtures/`, tested both on the Rust
  side and the Kotlin side.
- **Android package**: `io.github.hugoambiehl.padboard`.
- **i18n**: French by default, **no hard-coded UI text**. Android: `strings.xml`
  resources; desktop: `vue-i18n`. English to be added before open-sourcing.
- **CI (private repo, Windows minutes count double)**: Windows job only on push
  to `main` and on pull requests; cache Cargo, npm and Gradle; `concurrency`
  with `cancel-in-progress`.

## Conventions

- **Paths contain a space** (`Projets persos`): always quote paths in shell scripts,
  CI and config.
- Rust: `cargo fmt` + `cargo clippy -- -D warnings`. TS: ESLint + Prettier.
  Kotlin: ktlint.
- Tests: protocol round-trip serialization, configuration logic (migrations…).
- Config persisted as versioned JSON (with migrations) in the user config dir.
- Commits: small, **in English**, Conventional Commits
  (`feat|fix|docs|style|refactor|test|chore|perf(scope): summary`). No
  `Co-Authored-By` / "Generated with" lines.
- Never commit secrets, keystores or `local.properties`.
- Work phase by phase; ask before any significant architecture decision.

## Roadmap

1. **Foundations** — workspace, protocol + typeshare, minimal WS server in Tauri
   running a keyboard shortcut (testable on Linux), Android app connecting by manual
   IP and showing a fixed grid.
2. **Connection** — mDNS + PIN pairing, USB via adb reverse, auto-switch,
   reconnection, heartbeat.
3. **Editor** — replica, library, inspector, drag & drop, persistence, live sync,
   undo/redo.
4. **Actions** — all MVP actions (Linux + Windows), stateful buttons, folders,
   pages, multi-actions.
5. **Tablet polish** — launcher, lock task, animations, disconnected overlay,
   screensaver, performance.
6. **Distribution** — Windows installer, signed release APK, CI, README, open source.
