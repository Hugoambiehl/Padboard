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
- **USB** (tried first), port **47321**, plain `ws://`, bound to `127.0.0.1`
  only: the desktop follows devices with `adb track-devices` and runs
  `adb reverse tcp:47321 tcp:47321` on every (re)plug, **only on devices where
  Padboard is installed**; the tablet connects to `127.0.0.1:47321`. Connections
  on this port are authenticated without PIN (the cable is the proof). Accepted
  risk, documented in the README: any local process can connect to loopback too,
  so on a PC shared between several user accounts another account could send
  keys into the session.
- **`adb`**: from PATH or a location chosen in the settings. Downloading
  Platform-Tools automatically is **postponed to phase 6**: Google's binaries
  come under the Android SDK license, not a free license (to discuss then).
- **Wi-Fi**, port **47322**, **TLS (`wss://`)**, opt-in (see LAN default below):
  mDNS (`mdns-sd`) ↔ NsdManager. The desktop has its own long-lived self-signed
  certificate; the tablet **pins the SHA-256 of its public key (SPKI)**, not
  the certificate, so the certificate can be regenerated with the same key.
  Pairing: 6-digit PIN shown on desktop on demand (one-time, expires after
  2 min, 5 attempts in total then a new PIN is required) → token.
  The desktop pairing dialog and the tablet PIN screen both show the **short
  key fingerprint** (`crates/protocol/src/fingerprint.rs`, 60 bits,
  `XXXX-XXXX-XXXX`) for the user to compare. QR code pairing: backlog.
- **Tokens**: 32 bytes from the OS CSPRNG; the desktop stores only their
  SHA-256 and compares in constant time. Unauthenticated clients may only send
  `pair:request`. Paired devices are revocable from the status screen (live
  connection closed with `revoked`). The `Origin` check (403) applies to both
  ports. Tablet: token in DataStore, `allowBackup=false`, **one paired PC**.
- **Wi-Fi default (decided)**: off at first launch; turned on and remembered
  (`preferences.json`) when the user clicks "Activer le Wi-Fi" (pairing dialog)
  or the toggle of the status screen. `PADBOARD_BIND` (`lan` / `loopback`) still
  forces it, for development.
- Auto-reconnect with backoff, transparent USB ↔ Wi-Fi switch (make-before-break,
  the desktop closes the older connection with `superseded`), ping/pong
  heartbeat (latency shown in UI).

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
- Executors are **blocking** (a hotkey holds keys ~20 ms): call them from
  `tokio::task::spawn_blocking`. Platform code only implements low-level key
  down/up (`KeyInjector`); press/release sequencing is shared (`run_hotkey`)
  and always releases already-pressed keys on failure.
- Manual test: `cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT`.
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
- **Ports**: USB 47321 on `127.0.0.1` only (plain `ws://`, trusted), Wi-Fi
  47322 on `0.0.0.0` (TLS, pairing required), constants in `crates/protocol`.
  Wi-Fi is a persisted UI setting (`preferences.json`); `PADBOARD_BIND`
  (`lan` / `loopback`) forces it on or off, `PADBOARD_PORT` / `PADBOARD_WIFI_PORT`
  change the ports. Turning Wi-Fi off aborts the listener task, which owns and
  therefore closes every Wi-Fi connection.
- **Desktop config**: versioned JSON documents (`storage.rs`: step-by-step
  migrations, atomic writes, owner-only secrets, unreadable files moved aside)
  in Tauri's app config dir. The TLS private key is stored alone
  (`tls-key.pem`); the certificate is rebuilt from it at each launch.
- **Unauthenticated Wi-Fi clients are bounded**: connections before `hello`
  and connections in the pairing phase have separate caps and timeouts, so
  pairing attempts can never lock a paired tablet out.
- **Keys are stored by physical position**: W3C `KeyboardEvent.code` names
  (`KeyA`, `ControlLeft`…), mapped to evdev keycodes / Windows scancodes.
- **Generated types are committed**: `apps/desktop/src/generated/types.ts`
  (TS: protocol + actions + editable configuration) and the Android package
  `io.github.hugoambiehl.padboard.protocol` (Kotlin: protocol only). CI checks
  they are up to date.
- **Type generation**: `scripts/gen-types.sh` (`--check` in CI), config in
  `typeshare.toml`, typeshare-cli version pinned in the script. typeshare has no
  option to emit string unions, so it emits TS `enum`s: `erasableSyntaxOnly` is
  disabled in the desktop tsconfig for that reason. No custom post-processing of
  typeshare output.
- **Tauri versions**: the `tauri` crate and the `@tauri-apps/*` npm packages must
  stay on the same minor (currently `~2.11` on both sides); bump them together.
  Tauri plugins follow their own versions but require a minimum `@tauri-apps/api`:
  `tauri-plugin-dialog` / `@tauri-apps/plugin-dialog` stay on `~2.7` (2.8 needs 2.12).
- **Desktop dev launch**: `scripts/desktop-dev.sh`. It strips the GTK/GIO
  variables leaked by snap terminals (VS Code snap), which crash WebKitGTK.
- **WebSocket server**: rejects handshakes carrying an `Origin` header (browsers),
  so a web page on the PC cannot drive it; executes actions via `spawn_blocking`;
  is independent from Tauri (reports `ServerEvent`s, tested end-to-end in
  `server.rs`). UI state (`state.rs`) is mirrored by hand in
  `apps/desktop/src/types/status.ts`; the backend sends codes, the frontend
  translates.
- **Android dependencies**: every library must support minSdk 26 — check the
  `minSdkVersion` in its AAR manifest before adding it (the manifest merger and
  Android Lint `NewApi` check it again at build time). Kotlin is 2.2.10, so
  libraries must be built with Kotlin ≤ 2.2/2.3 (kotlinx-serialization stays on
  1.9.x until Kotlin is upgraded). Versions only in `gradle/libs.versions.toml`.
- **Android code style**: ktlint (`android_studio` style, `.editorconfig`), the
  generated `protocol/Protocol.kt` is excluded. Logging through
  `java.util.logging` so non-UI code stays testable on the JVM.
- **Tablet connection** (`connection/ConnectionManager.kt`): one manager owns the
  link, on a single-threaded dispatcher (no locks). USB first (no token sent
  outside TLS), then the paired PC's addresses in order (mDNS, then the last
  that worked), accepted only with the right `serverId` over the pinned key.
  USB is probed while on Wi-Fi and taken over before Wi-Fi closes
  (`superseded` from the PC is expected then). The TLS key is captured by the
  trust manager (`Pinning.kt`): OkHttp does not expose the handshake of a
  WebSocket upgrade. mDNS: Android < 14 resolves one address per service, so
  the desktop announces IPv4 only and the tablet accumulates addresses.
- **Desktop session liveness**: tablets ping every 5 s; the desktop closes an
  authenticated session silent for 20 s.
- **Tablet press feedback**: `button:press` on touch down, `button:release` on
  lift, released in a `finally` so a cancelled gesture never leaves a key held.
- **Manual tests without the tablet**: the `fake-tablet` example of the desktop
  crate implements the tablet side (discovery, pinning, pairing, token); its
  pinning logic (`pinning.rs`) is the reference for the Android implementation.
- **Shared JSON fixtures** in `crates/protocol/fixtures/`, tested both on the Rust
  side and the Kotlin side.
- **Android package**: `io.github.hugoambiehl.padboard`.
- **i18n**: French by default, **no hard-coded UI text**. Android: `strings.xml`
  resources; desktop: `vue-i18n`. English to be added before open-sourcing.
- **CI (private repo, Windows minutes count double)**: `.github/workflows/ci.yml`.
  Windows job only on push to `main`, pull requests and manual runs
  (`workflow_dispatch`); cache Cargo, npm and Gradle; `concurrency` with
  `cancel-in-progress`. The frontend is built before any Rust job (the Tauri
  crate embeds `apps/desktop/dist`). `setup-gradle` uses `cache-provider: basic`
  (MIT) — its default provider is proprietary. Bump `TYPESHARE_VERSION` in the
  workflow together with `scripts/gen-types.sh`. A CI change is validated on a
  branch (no Windows job) before reaching `main`.

## Editor (phase 3) decisions

- **Rust is the single source of truth**: the configuration (`config/`,
  `profiles.json`) is only changed through edit operations sent by the
  frontend, validated, then persisted (debounced) and pushed to the tablets.
  Undo/redo lives in Rust (snapshots); successive edits of one field (typing a
  label) merge into one undo entry; layout pushes are throttled to one every
  100–150 ms while typing.
- **Icons**: Lucide (ISC license), stored as `{ name, paths }`: the name for the
  picker, the SVG path data for the tablet (protocol `ButtonIcon`). The frontend
  converts Lucide elements to paths (`src/icons/paths.ts`); the desktop
  validates them (count, size, syntax) before sending. User images: backlog
  (phase 4 or 5).
- **Pages** are identified by id, never by position. If the page shown on the
  tablet is deleted, the tablet falls back to an existing page.
- **Library**: only working actions; shortcut presets are adapted to the OS
  (lock: Win+L / Super+L; screenshot: Win+Shift+S / Print Screen).
- **`Action`** is adjacently tagged (`{ type, params }`) so typeshare can
  generate it. `Key` comes from a macro typeshare cannot see: keys are strings
  in TS and the editor gets the valid names from the backend.
- **Drag and drop** is hand-made with pointer events (WebKitGTK and WebView2
  behave differently with HTML5 DnD), with a keyboard alternative to move the
  selected button. Frontend logic is tested with Vitest.
- **Editor frontend**: `useEditor()` is a read-only mirror of the Rust state
  (`editor` event); every change goes through `edit()`. Text fields bound to
  the configuration use `DraftInput` (keeps its draft while focused, since
  edits round-trip through Rust). UI previews without the backend:
  `npm run dev`, then `http://localhost:1420/?mock` (`src/dev/mock.ts`,
  development only; `&view=connections`, `&select=row,col`).
- **Lucide upgrades**: run `node scripts/export-icons.mts` then the ignored
  Rust test `all_lucide_icons_are_valid` (see `svg_path.rs`), so every icon
  still passes the desktop's path validator.

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

1. **Foundations** ✅ — workspace, protocol + typeshare, minimal WS server in Tauri
   running a keyboard shortcut (testable on Linux), Android app connecting by manual
   IP and showing a fixed grid.
2. **Connection** ✅ — mDNS + PIN pairing, USB via adb reverse, auto-switch,
   reconnection, heartbeat.
3. **Editor** — replica, library, inspector, drag & drop, persistence, live sync,
   undo/redo. Backlog: dark window title bar (GNOME shows a light one while
   the app is dark) — window `theme: "Dark"` or a custom title bar.
4. **Actions** — all MVP actions (Linux + Windows), stateful buttons, folders,
   pages, multi-actions.
5. **Tablet polish** — launcher, lock task, animations, disconnected overlay,
   screensaver, performance. Backlog: the Android 12L taskbar stays drawn over
   the app in immersive mode on the SM-T580 (no inset reported); QR code pairing.
6. **Distribution** — Windows installer, signed release APK, CI, README, open source.
   License notices to ship: Lucide icons (ISC), Inter and Space Grotesk (OFL).
