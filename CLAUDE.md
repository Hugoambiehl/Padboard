# Padboard

DIY Stream Deck: a **desktop app** (Tauri 2) that executes actions on the PC, and
an **Android app** (Kotlin + Compose) used as a touch control surface on a dedicated
tablet. Public, open source repo (since v0.1.0).

**Absolute rule: only free and open-source tools, libraries and APIs.** No paid API,
nothing that requires approval, no Google Play Services / Firebase / ML Kit.
**One exception, decided by Hugo for v0.2**: on Windows, "Installer adb"
downloads Google's Platform-Tools (not free, Android SDK License) on the
user's request, after an honest acceptance dialog (link to the license,
§3.1 and §3.4 summed up, "Padboard n'est pas affilié à Google"). Never
bundled or redistributed, always optional (Wi-Fi works without adb),
version + SHA-256 pinned in the code (fallback for an archive Google
republished under the same version: Google's index, same size + SHA-1,
#18) and every `.exe`/`.dll` checked for a valid "Google LLC" Authenticode
signature; listed in the app's
"Licences" dialog as downloaded on request.
**Second exception, decided by Hugo for 0.5 (#20)**: the second screen on
Windows uses [Virtual Display Driver](https://github.com/VirtualDrivers/Virtual-Display-Driver)
(MIT, parts MS-PL from Microsoft's IndirectDisplay sample) despite the
open license question of its ancestor `roshkins/IddSampleDriver` (no
license file, ~40 lines; risk judged very low, no answer awaited). To
limit that risk it is **never bundled** in Padboard's installer:
downloaded on demand from the official signed release of
`VirtualDrivers/Virtual-Display-Driver`, like "Installer adb" (version +
SHA-256 pinned in the code, SignPath Foundation Authenticode signature
checked, honest dialog first naming MIT and MS-PL and saying it is a
separate project), then installed with an administrator prompt;
listed in the "Licences" dialog as downloaded on request.
No other exception.

**One install only (durable rule, decided by Hugo).** The user installs
Padboard and nothing else. Every feature or integration must work without
any other software installed beside it: the components it needs are built
into Padboard, or downloaded and installed by Padboard itself on demand
(like "Installer adb"), always free and open source, unless Hugo decides
an exception.

## Targets

- **Desktop**: Windows 10/11 (main target), Linux (dev + local tests, with real
  Linux implementations of actions, not stubs). Dev machine: Ubuntu 26.04, GNOME
  **Wayland**. Windows installers are built by GitHub Actions on a Windows runner.
- **Tablet**: Samsung Galaxy Tab A 10.1 (SM-T580), 2 GB RAM, 1920×1200
  (≈ 1280×800 dp), landscape. LineageOS 19.1 = Android 12L (API 32), **no Play
  Services, no vibration motor**. `minSdk 24` (Android 7.0, since 0.3: issue #21), `targetSdk` = latest stable.

## Stack

- **Desktop**: Tauri 2. Rust backend; frontend Vue 3 + TypeScript + Tailwind,
  Motion for Vue (GSAP if needed), Lucide icons. Tray icon (Tauri), autostart
  (`auto-launch`) and single instance (Tauri plugin).
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
  on this port are authenticated without PIN (the cable is the proof). Since
  0.5 (#20, `usb_peer/`), each one must have been **opened by the adb server**:
  the process owning its other end (Linux `/proc/net/tcp` + inode → PID, or
  the socket owners' UIDs when the server runs under another account, e.g.
  `sudo`; Windows `GetExtendedTcpTable`) must be the one listening on the adb
  server's port (`ANDROID_ADB_SERVER_PORT`, default 5037). Not adb = refused,
  logged with the program, shown 5 min in the USB card (`Snapshot.usb_refused`);
  impossible to check = refused too, the tablet told why
  (`ErrorReason::UsbUnverified`, `usbRefused` notice) and the card says what
  to do. Off in simulated test mode (`UsbPeers::Any`: fake-tablet, e2e) and
  unit tests; on with `PADBOARD_TEST_SYSTEM=1`. What remains (SECURITY.md):
  the adb server accepts commands from any local account.
- **`adb`**: a location chosen in the settings, else `PATH`, else the
  Android SDK folders, else the copy downloaded by "Installer adb"
  (Windows, `platform_tools/`: Platform-Tools pinned in `mod.rs`, bumped
  with `scripts/platform-tools-pin.sh`; Authenticode check in
  `authenticode.rs`, Windows-only, cross-checked by CI's Windows job;
  extracted to `%LOCALAPPDATA%\io.github.hugoambiehl.padboard\platform-tools`;
  a 4xx from Google = `unavailable`, the UI then offers the manual
  choice; a download that is not the pinned archive is logged in detail
  (sizes, SHA-256s, status, type, final URL, first bytes) and told apart
  (`Mismatch`: web page, cut, another zip, other), with the manual choice
  too. Google republishes an archive under the same version (37.0.3 on
  2026-11-19: every download refused): `platform-tools-pin.sh --check`
  compares the index's size and SHA-1 with the pin, weekly in CI). Since
  0.4 (#18) a whole zip that is not the pinned one is accepted when
  `repository2-3.xml` (`INDEX_URL`, read with quick-xml) lists, for the
  pinned version, the pinned file name with that size and SHA-1; never
  another version; Authenticode unchanged; logged at WARN ("NOT the pinned
  archive, accepted from Google's index", both SHA-256s, the SHA-1),
  recorded next to the tools (`padboard-from-index.json`) and shown in
  orange in the USB card (`PlatformToolsInfo.fromIndex`). Linux: the `.deb` depends on the distribution's free `adb`
  package (`bundle.linux.deb.depends`), which recommends the udev rules.
  Windows drivers: not known whether Windows 11 needs one for the SM-T580
  (the test PC had Samsung's driver, installed by something else): the
  docs never claim Windows does without, Samsung's driver is the
  documented fallback (`help.rs` links the guides). "Supprimer adb" stops
  the adb server only if its process (`platform_tools/server_owner.rs`:
  listener of port 5037, its executable) is our copy's `adb.exe`:
  another program's server (Touch Portal's adb 28.0.3, same protocol 41)
  keeps running. Since 0.5 (#35) it puts adb on hold first
  (`Backend.adb_hold`: the tracker drops its session and starts nothing,
  kiosk and USB update commands answer busy), waits up to 3 s for our
  server to exit, then `platform_tools/removal.rs` moves the folder
  aside (`platform-tools.removing`, retried ~5 s while a file is in use)
  and deletes it: the copy is either gone or whole; still in use =
  `PlatformToolsError::InUse` (clear message, not the OS error). A
  leftover is deleted at the next removal or installation.
  **Several adb on one PC** (adb.rs): they share one server (port 5037);
  different server versions (`adb version` "1.0.N") stop each other's
  server. Before any command Padboard asks the server its version
  (`host:version`): another version = `AdbState::Conflict`, server left
  alone (no `start-server`, no command), retried. No server: wait
  `START_GRACE` (1 s) and ask again before `start-server` (two servers
  starting together: one aborts, Ubuntu reports a crash). 3 server stops
  within a minute = `UsbStatus.server_unstable`. Docs: "Several adb on the
  same PC" in install-linux.md / install-windows.md. Seen on Hugo's PC:
  Ubuntu's adb 34 and the SDK's 37 are both 1.0.41 and coexist.
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
docs/install-*.md      # user guides (Windows, Linux, tablet incl. kiosk mode and rescue)
docs/development.md    # build, run, test (README is user-facing)
docs/windows-test-plan.md # Windows per-action manual test plan
docs/tablet-test-plan.md  # manual checks on the real tablet (system gestures, kiosk)
docs/release-signing.md # release keys (tablet APK, desktop updates), outside the repo
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
- **Windows** (`crates/actions/src/win/`, `windows` crate): `SendInput`;
  volume and microphone with `IAudioEndpointVolume` on the default playback
  and recording devices (master scalar = the Windows slider); media through
  the system media session (GSMTC), media keys as a fallback; `ShellExecuteW`
  (environment variables expanded) and `LockWorkStation`. The audio state is
  polled every 300 ms (only real changes reported). Check locally with
  `cargo clippy -p padboard-actions --target x86_64-pc-windows-msvc`.
- **Linux** (`crates/actions/src/linux/`): keyboard injection (uinput);
  volume and microphone through the PulseAudio native protocol with the pure
  Rust `pulseaudio` crate (PipeWire serves it via `pipewire-pulse`; no
  libpulse to build or run), one short connection per call; media through
  MPRIS (`zbus` 5.x blocking API, 6.x needs Rust 1.89): the playing player,
  else a paused one, else the media key; `open`: URLs and documents with
  `xdg-open`, `.desktop` entries with `gio launch`, programs started directly
  (no shell, own process group); lock with `loginctl lock-session`.
- An action with no Linux equivalent must **log clearly and return an error**,
  never panic.
- Executors are **blocking** (a hotkey holds keys ~20 ms): call them from
  `tokio::task::spawn_blocking`. Platform code only implements low-level key
  down/up (`KeyInjector`); press/release sequencing is shared (`run_hotkey`)
  and always releases already-pressed keys on failure.
- **Action kinds**: system ones (hotkey, media, volume, microphone, open,
  lock) run in an `ActionExecutor`; navigation, multi actions and OBS are
  carried out by the desktop app. `Action::validate` (before running) is
  strict; `validate_draft` (configuration) accepts values not filled in yet.
- **Observed state** (`padboard_actions::state`): audio state, audio change
  feed, media playing; read by the desktop to show button states.
- Manual tests: `cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT`,
  `--example action -- '<action JSON>'`, `--example state [-- --watch]`.
- **OBS** (`apps/desktop/src-tauri/src/obs.rs`): our own obs-websocket v5
  client (JSON over the app's tokio-tungstenite; `obws` needs Rust 1.89).
  Settings in `obs.json` (owner-only, holds the password), edited in the
  connections view. One task owns the connection, retries every 3 s while
  OBS is closed (buttons unavailable meanwhile), waits for new settings after
  a wrong password, publishes scene / recording / streaming / muted inputs
  once all initial answers are in (OBS answers in any order).
  **Never ask OBS anything while it starts** (issue #14: OBS 33 froze on
  Windows, `AppHangB1`, when Padboard was already running): obs-websocket
  accepts clients during the load, answers 207 until the scene collection
  is loaded, then serves requests while OBS's UI thread is still in
  `OBSInit` (until `FINISHED_LOADING`, invisible to clients), a window
  where requests can deadlock OBS. Each session first asks `GetVersion`:
  207 = disconnect, retry in 3 s; when OBS was closed or loading at the
  last attempt, that check connects subscribed to no event and the real
  session comes `STARTUP_SETTLE` (10 s) later. Mute states: one
  `RequestBatch`. Attempts and initial requests logged at INFO (a
  repeated failure at DEBUG). Tests use an
  isolated OBS: `XDG_CONFIG_HOME=<dir> xvfb-run obs` with obs-websocket on
  another port and its recording path in that dir. The editor's scene and
  input lists (`get_obs`) are re-read on the `obs` event, sent when the
  status changes **and** when the names in `SystemState` change (they
  arrive after `Connected`, issue #13); a scene collection change asks
  them again. e2e: `e2e/obs` with the `fake-obs` example
  (`PADBOARD_TEST_OBS=1`: the real client in simulated test mode).
- Discord only via its global shortcuts (no RPC).

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
  The udev rule granting access to `/dev/uinput` is documented in `docs/install-linux.md`.
- **Ports**: USB 47321 on `127.0.0.1` only (plain `ws://`, trusted), Wi-Fi
  47322 on `0.0.0.0` (TLS, pairing required), constants in `crates/protocol`.
  Wi-Fi is a persisted UI setting (`preferences.json`); `PADBOARD_BIND`
  (`lan` / `loopback`) forces it on or off, `PADBOARD_PORT` / `PADBOARD_WIFI_PORT`
  change the ports. Turning Wi-Fi off aborts the listener task, which owns and
  therefore closes every Wi-Fi connection.
- **Second screen on Linux: GStreamer** (`screen/gst.rs`, #20): `gstreamer`
  / `gstreamer-app` crates **~0.24** (0.25 needs Rust 1.92; Dependabot
  ignores it), Linux-only dependencies (Windows: Media Foundation, never
  GStreamer), **no `v1_*` feature** (the .deb is built on Ubuntu 22.04,
  GStreamer 1.20). Encoders: `vah264enc` (VA-API, GStreamer 1.22+, in
  `gstreamer1.0-plugins-extra` on recent Ubuntu, `-bad` before), else VP8
  (`vp8enc`, plugins-good); never x264. Frames are dropped before the
  encoder (an `identity` gate whose probe holds `ENCODER_AHEAD` = 2
  credits, one back per frame the stream takes; a valve per requested
  frame capped H.264 at ~49 fps). The screen (`screen/portal.rs`):
  ScreenCast portal over the desktop's `zbus` (blocking), a `VIRTUAL`
  monitor sized by the caps after `pipewiresrc` (`fd` + `path`, not
  `target-object`: 22.04's PipeWire), cursor embedded, persist mode 2,
  restore token owner-only in `screen-cast-token` (single use: replaced
  each time), `keepalive-time` 500 ms. Starting may wait for the PC's
  dialog: `offer_screen` is async, the source starts on a blocking
  thread outside the sessions lock, a press meanwhile is ignored
  (`Screens::starting`). Measured at 60 fps on the real screen: H.264
  8 % of a core, VP8 51 % (`docs/design/second-screen.md`). The .deb depends on
  libgstreamer, plugins-base, plugins-good and gstreamer1.0-pipewire
  (already on an Ubuntu desktop: nothing added) and only *recommends*
  plugins-extra: plugins-bad was measured at +121–144 MiB (x264, x265…)
  on 22.04 / 24.04 desktops, so it is never a dependency. CI's "Linux
  package" job builds the .deb on 22.04 and starts it in a clean 22.04
  container (`scripts/deb-smoke-test.sh`).
- **Second screen action** (`Action::SecondScreen`, run by the desktop
  in `server::press`, never a multi step nor "Tester l'action"): offers
  a stream (`ServerContext::offer_screen`, `screen/`) to the tablet that
  pressed. Listed in the library only where `ServerContext.screen_source`
  exists: for now only on request, `PADBOARD_SCREEN_SOURCE=screen`
  (virtual monitor) or `pattern` (Linux), until the tablet shows it. While it runs the header and the tray show it with
  "Arrêter" (`Snapshot.second_screen`, `stop_second_screen`).
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
  The NSIS installer uses our template (`src-tauri/windows/installer.nsi`:
  Tauri's own of the CLI in use, `; Base:` line checked by
  `scripts/tests/test_nsis_template.py`, changes marked `; Padboard:`):
  the installed version is always uninstalled with `/UPDATE /P` (Tauri's
  plain uninstall offered "Delete the application data" during an update,
  and removed the `Run` value), and the directory page is skipped when
  Padboard is installed (issue #12). A CLI bump means re-applying them
  (`docs/releasing.md`). Header and sidebar BMPs come from `icons.mjs`.
  Tauri plugins follow their own versions but require a minimum `@tauri-apps/api`:
  `tauri-plugin-dialog` / `@tauri-apps/plugin-dialog` stay on `~2.7` (2.8 needs 2.12).
- **Desktop dev launch**: `scripts/desktop-dev.sh`. It strips the GTK/GIO
  variables leaked by snap terminals (VS Code snap), which crash WebKitGTK.
- **Android 17 local network** (#31, decided by Hugo: permission at the
  right moment): targeting API 37, Wi-Fi to the PC and mDNS need
  `ACCESS_LOCAL_NETWORK` (runtime, "Nearby devices" group); USB
  (loopback through `adb reverse`) does not (checked on the API 37
  emulator). Without it a plain `discoverServices` opened the system's
  `NsdPickerActivity` over the app at every launch. `LocalNetwork`
  (`connection/LocalNetwork.kt`) holds the state; mDNS runs only while
  allowed (`TabletLink`), `ConnectionManager` skips Wi-Fi without it
  (`LinkState.Searching.localNetwork`). Asked for only when Wi-Fi is
  wanted: the setup screen (pair over Wi-Fi) and the disconnected card /
  searching screen of a Wi-Fi-paired tablet show "Autoriser le Wi-Fi";
  refused for good, the button opens the app's settings. Kiosk mode
  grants it itself (`setPermissionGrantState` in `Kiosk.apply`). The
  system picker path (`FLAG_SHOW_PICKER`) was not chosen: no typed IP, no
  last address, a rewrite. `LocalNetworkTest` (API 37 only, on demand:
  see its doc) fails without the fix.
- **Status in the UI**: every server event sends the whole `Snapshot`;
  `useStatus` merges it with `reconcile` (`composables/reconcile.ts`):
  unchanged parts keep their identity, so a tablet reconnecting only
  redraws what changed (a supersede loop made the whole connections view
  render twice a second on Windows).
- **WebSocket server**: rejects handshakes carrying an `Origin` header (browsers),
  so a web page on the PC cannot drive it; executes actions via `spawn_blocking`;
  is independent from Tauri (reports `ServerEvent`s, tested end-to-end in
  `server.rs`). UI state (`state.rs`) is mirrored by hand in
  `apps/desktop/src/types/status.ts`; the backend sends codes, the frontend
  translates.
- **Android dependencies**: every library must support minSdk 24 — check the
  `minSdkVersion` in its AAR manifest before adding it (the manifest merger and
  Android Lint `NewApi` check it again at build time), and its license against
  the Licensee allow-list. Kotlin is 2.2.10, so
  libraries must be built with Kotlin ≤ 2.2/2.3 (kotlinx-serialization stays on
  1.9.x until Kotlin is upgraded). Versions only in `gradle/libs.versions.toml`.
  **API 24, no desugaring** (decided for 0.3, #21: `desugar_jdk_libs` would
  add a license for two calls): no `java.time` or `java.util.Base64` (API
  26) in the app; `kotlin.io.encoding.Base64`, `java.text` / `Calendar`
  instead; Lint `NewApi` stops any other. JVM tests may use them.
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
- **Wi-Fi reachability**: PC and tablet must reach each other directly;
  guest networks, client isolation, repeaters and some meshes prevent it
  (seen on a Starlink mesh: 60–78 % ping loss, sessions dropped). The
  tablet says so when the paired PC is announced by mDNS but does not
  answer (`LinkState.Searching.announced`); the PC when a Wi-Fi session
  goes silent and is dropped (`ClientDisconnected.lost` →
  `Snapshot.dropped_wifi`, shown 5 min). Updates over Wi-Fi are paced
  (`Config.updatePiecePause`, 250 ms between 64 KiB pieces); a cut
  transfer keeps nothing and says so. Docs: install-tablet.md.
- **Desktop session liveness**: tablets ping every 5 s; the desktop closes an
  authenticated session silent for 20 s.
- **Button states** (`appearance.rs`, `observe.rs`): watcher threads publish
  the observed `SystemState` (audio change feed, media polled every second,
  only real changes); each session resolves its layout with it, sends it
  whole on connect / edit (so a new tablet gets every state) and otherwise a
  `button:update` only for buttons whose resolved look differs from what it
  last sent. Unknown state (test mode, not observed yet) keeps the resting
  look; an unreachable source makes the buttons `unavailable`. Tablet texts
  of the states come from `locale.rs` (the PC's language).
- **Tablet presses vs page swipes**: the tile reacts (scale, flash) on touch
  down, but `button:press` is sent only once the touch is not a swipe: on lift
  without moving past the touch slop (tap), or after 120 ms held still (hold).
  A touch that moves first belongs to the pager and sends nothing.
  `button:release` follows on lift or when the pager takes over, in a
  `finally`, so a cancelled gesture never leaves a key held.
- **Multi actions** (`server::perform`): steps run in order, a failed step
  stops the sequence ("step N: …" to the tablet), pauses up to 10 s, steps
  cannot be multi actions. A press starts its own task (the session keeps
  answering pings and other buttons); a second press of the same button
  while it runs is ignored. Navigation steps go to the pressing tablet
  through the session's outbox; "Tester l'action" skips them.
- **Navigation and folders**: a navigation button is not run on the PC: the
  desktop answers the pressing tablet with `navigate`. Folders are pages
  outside the swipe order (`ProfileConfig.folders`), edited in the same tabs;
  `AddFolder` creates the folder and its button in one undo entry; a folder
  button without its own subtitle shows "Dossier · N boutons". Deleting a
  folder keeps the buttons that opened it (flagged in the inspector).
- **Tablet pages**: `HorizontalPager` keyed by page id; the page shown is
  followed by id across layout updates, reconnections and USB ↔ Wi-Fi
  switches (`PageMemory` in `ui/grid/PageTracking.kt`, owned by the
  ViewModel; applied during composition with `requestScrollToPage`, so no
  wrong frame). Another profile starts on its first page; a page deleted live
  gives way to the page at its position; a page gone while disconnected, or an
  app restart, starts on the first page. The page indicator is drawn in a `Canvas` from the pager
  position (draw phase only), hidden for a single page. The `navigate`
  collector never waits for an animation (a scroll that cannot finish, e.g.
  pages hidden by a folder, blocked every later request): each scroll runs
  in its own coroutine, the next one cancels it.
- **Tablet rendering cost** (measured in release on the SM-T580 with
  `dumpsys gfxinfo framestats`): each page grid and the folder grid are
  drawn into a cached offscreen layer (`CompositingStrategy.Offscreen`), so
  swipes and fades only move or fade a texture (swipe frames 45 → ~31 ms,
  UI thread 14 → ~6 ms). The pager stays composed while a folder is open;
  the folder grid is an overlay faded in over it, kept composed once opened
  and simply not placed while closed (composing either on each open/close
  dropped frames). Measure with a release APK signed locally with the debug
  key (`apksigner`), never committed.
- **Install identity, never change it**: `bundle.publisher` ("Hugo Ambiehl"),
  `identifier` (`io.github.hugoambiehl.padboard`) and `productName`
  ("Padboard") in `tauri.conf.json`. The NSIS installer finds the previous
  install under `HKCU\Software\<publisher>\<productName>` and the config
  dir is named after the identifier: changing one makes an update install
  beside the old version (seen from rc.3 to rc.4, when the publisher left
  "github") or lose its configuration.
- **Release signing**: one key outside the repo (`~/.padboard-signing/`,
  pointed to by `padboard.signing` in `~/.gradle/gradle.properties`); the
  tablet runs the release build. Never change the key: a device owner app
  cannot be uninstalled (see `docs/install-tablet.md`, kiosk mode).
- **Kiosk mode** (`kiosk/Kiosk.kt`): the app is the device owner (set with
  `adb shell dpm set-device-owner`, from the PC's USB card), locks itself in
  (lock task, only the power menu), is the home app (`HomeAlias`, enabled
  only in kiosk mode), lock screen and status bar off, screen kept on.
  The LineageOS taskbar is turned off by the same PC command
  (`enable_taskbar`, a LineageOS setting the app cannot write). Leaving it
  always removes the device owner (`clearDeviceOwnerApp`): from the PC
  (`tablet:command exit_kiosk`, "Libérer la tablette": two clicks, the
  second at least 1 s after the first, `releaseClick` in `usb/kiosk.ts`,
  so a double-click only arms it) or by holding the
  bottom-left corner (`KIOSK_EXIT_CORNER`, 40 dp) 5 s on any screen
  (touches observed, never consumed), then confirming (red button armed
  after 2 s; cancel by default, closes alone after 30 s). Every screen
  keeps that margin empty, checked by `KioskCornerTest` (androidTest,
  `scripts/tablet-ui-tests.sh`: throwaway emulator at the tablet's size).
  The top-right corner was used up to rc.1: next to the connection pill,
  it was triggered during a pairing. The tablet reports kiosk mode with
  `tablet:status`; who left it (`KioskExitOrigin`: gesture / PC) is
  logged on the tablet, kept on disk (`KioskExits`) and sent once in the
  next `tablet:status` (`kioskLeft`, optional field), logged by the PC.
  The PC's USB row describes the gesture of the version installed on
  the tablet (`usb/kiosk.ts` `kioskActiveKey`: top-right below rc.2,
  versionCode 20002). The PC's USB row reads the
  device owner with `dpm list-owners` (on detection, after "Activer", on
  every `tablet:status`; Android 7.0 has no such command, prints "unknown
  command" with a success status: then `dumpsys device_policy`'s `Device
  Owner:` section): already Padboard = success, another owner = a
  clear refusal (codes in `adb::KioskError`, translated by the UI).
- **Kiosk mode and updates**: an `adb install -r` stops the app and the
  tablet would show the regular launcher; `KioskUpdated`
  (`MY_PACKAGE_REPLACED`) brings it back in front at once, as the home
  entry (a device owner may start an activity from the background).
- **One screen, one connection** (0.4.0-rc.2; rc.1 loop on the SM-T580):
  Android 12L kept two `MainActivity` instances in kiosk mode (the PC's
  `am start` in a standard task, `HomeAlias` started by the system at the
  first wake-up in a home task), each with its own connection: the PC saw
  the tablet supersede itself every 0.52 s. Now the connection belongs to
  the process (`connection/TabletLink.kt`, reference-counted by the
  ViewModels); kiosk mode runs in the home entry only: a screen that is
  not the home entry hands over to it (`Kiosk.homeIntent`, once per
  screen, without locking) and is closed by it; **no `lockTaskMode` in
  the manifest** (it locked the PC's `am start` too, and Android never
  lets the root of a lock task finish; a closed home entry is started
  again at once by the system). Android 7 hands the home intent to the
  same instance, which then locks itself. Leaving kiosk mode from the
  home entry: it finishes, reopens `MainActivity` in a normal task, and
  the alias is disabled in its `onDestroy` (disabling it first closes the
  screen: the launcher came in front); `Kiosk.refresh` disables a
  leftover alias. A session superseded by another of the same tablet (not
  our USB probe) waits `supersededPause` (5 s, doubled up to 2 min) before
  taking the PC back; the PC warns *keeps replacing its own session*
  (5 in a minute, `auth.rs` `SupersedeWatch`). `KioskSingleInstanceTest`
  (androidTest) fails without the fix on API 32 only (image
  `android-32;google_apis`, local test tool); skipped on the API 37
  emulator (task switches crash its system_server, at random: any test
  that starts a task there is on demand). Kiosk mode left while a home
  screen is alive but not in front waits for it (`Kiosk.homeScreens`,
  #32; with the screen off the home screen stays resumed, the keyguard
  being off).
- **Tablet screensaver** (`ui/screensaver/Screensaver.kt`): settings stored
  on the PC (`preferences.json`, "Écran de veille" card, 15 s–1 h, default
  on after 5 min), sent with `tablet:settings` on connect and on change;
  the tablet keeps the last ones (defaults before the first connection).
  Touches are observed on the way down (never consumed); asleep: black
  overlay with the time, the date and `system:status` (microphone muted,
  OBS REC / EN DIRECT, "PC non connecté" without a PC), window brightness
  0.02, clock drifting every minute (`graphicsLayer` translation). The
  waking touch is consumed whole: it never reaches the grid.
  `system:status` is derived from `SystemState` (unknown = off) and sent
  only when it changes.
- **Protocol 5, the frozen core**: `hello`, `welcome`, `error`,
  `ping`/`pong` and `update:*` (+ the APK's binary frames: 4-byte
  big-endian offset, then bytes) never break, whatever the protocol
  version: only optional fields are added (`SessionMode` never gains a
  value). Every version's core is kept in `crates/protocol/fixtures/core/`
  (`<type>-v<N>.json`, never edited; copy the new ones when bumping),
  decoded by Rust and Kotlin tests. Handshake: protocol ≤ 4 is rejected
  (a paired one is reported to the UI, `OutdatedTablet`: USB updates it);
  another version with the core is checked **after** authentication and
  served as an **update-only** session (`welcome.mode`): no layout,
  settings, states or orders, only pongs and `update:*`; anything else
  gets `unauthorized` and closes it (tests in `server.rs`). No pairing
  across versions. `hello` carries `appVersion`, `appVersionCode` and
  `silentInstall` (device owner). Updates over the connection go only to
  `silentInstall` tablets with an older `versionCode`
  (`ServerContext::offer_update`), in 64 KiB pieces the tablet pulls one at
  a time (`tablet_update.rs`).
- **Protocol version mirrored by hand** in `ProtocolJson.kt`: the hello /
  welcome fixtures carry the version and both sides' tests compare to them.
- **Tablet disconnected**: the grid stays (same `GridScreen` call site, so
  the pager survives) and, after a 1 s grace period (USB ↔ Wi-Fi hand-over,
  blips), dims under a card: PC name, how it is looked for, next attempt,
  "Réessayer maintenant" (`nudge`). A tablet never paired over Wi-Fi keeps
  looking for the PC last reached over USB (`Searching(wifi = false)`)
  instead of falling back to the setup screen.
- **Tablet animations**: a tile's look (icon, color, texts, badge) changes
  through a `Crossfade` (alpha); presses scale to 0.92 with a stronger
  flash and spring back (no vibration motor); a folder fades in from a
  0.94 scale (220 ms; scaling the pager behind it cost frames, dropped);
  the grid fades and scales in once, on the first layout. Measured on the
  SM-T580 (release, frame time to `SwapBuffers`, as `FrameCompleted` is not
  filled on this ROM): folders p50 32 → 28 ms, same count of frames over
  33 ms; swipes unchanged.
- **Tablet at rest draws nothing**: no animation may run forever. The
  search spinner and the reconnecting dot animate during the first
  `SPINNING_ATTEMPTS` (≈ 20 s) only, then stand still; hidden overlays are
  not composed (a hidden spinner still asked for 60 frames a second: 90 %
  CPU on the SM-T580 while the PC was off). Check with
  `dumpsys gfxinfo <pkg>` (reset, wait 5 s): connected and idle ≈ 2
  frames, waiting for the PC ≈ 5.
- **Tablet startup** (release, SM-T580): first frame ≈ 680 ms
  (`am start -W`, measurable only outside kiosk mode: a lock task app
  cannot be force-stopped); the first grid then takes ≈ 1.3 s of main
  thread once (first composition of the grid screen), whatever the ART
  compilation (`verify`, `speed-profile`, `speed`: same numbers), so a
  hand-written Baseline Profile, font preloading and a hidden warm-up tile
  were measured and dropped (no gain). Memory ≈ 100 MB PSS, two thirds of
  it the cached page layers.
- **Tablet icons**: the protocol's SVG paths become an `ImageVector` (Lucide
  24×24 grid, 2-unit round strokes, drawn white and tinted), cached by paths
  (`ui/grid/IconVectors.kt`) so layout updates never rebuild an unchanged
  icon; an unreadable icon falls back to the label's initial.
- **Manual tests without the tablet**: the `fake-tablet` example of the desktop
  crate implements the tablet side (discovery, pinning, pairing, token); its
  pinning logic (`pinning.rs`) is the reference for the Android implementation.
- **Shared JSON fixtures** in `crates/protocol/fixtures/`, tested both on the Rust
  side and the Kotlin side.
- **Android package**: `io.github.hugoambiehl.padboard`.
- **i18n**: English and French, **no hard-coded UI text**. Desktop: the
  PC's language (`locale.rs`: French if the system language is French,
  English otherwise; `PADBOARD_LOCALE=fr|en` forces it, e2e tests run in
  French) decides both the editor (`get_locale`, vue-i18n with
  `locales/fr.json` + `en.json`, same keys / parameters / plural forms
  checked by `locales.test.ts`) and the texts sent to tablets (button states,
  folder subtitles, the demo profile created at first launch). Tablet:
  Android's language, English in `values/`, French in `values-fr/`.
- **CI** (set up while the repo was private, Windows minutes counting
  double): `.github/workflows/ci.yml`.
  Windows job only on push to `main`, pull requests and manual runs
  (`workflow_dispatch`); cache Cargo, npm and Gradle; `concurrency` with
  `cancel-in-progress`. The frontend is built before any Rust job (the Tauri
  crate embeds `apps/desktop/dist`). `setup-gradle` uses `cache-provider: basic`
  (MIT) — its default provider is proprietary. Bump `TYPESHARE_VERSION` in the
  workflow together with `scripts/gen-types.sh`. A CI change is validated on a
  branch (no Windows job) before reaching `main`.
  Gradle steps (CI and release APK) go through `scripts/gradle-retry.sh`:
  run again, up to 3 times, only when a repository did not answer (Maven
  Central rate-limits GitHub's runners with 429s; #22). Retries add
  `--refresh-dependencies` (Gradle caches a module's absence: retries on
  the same runner never passed) and `--info`, whose failed HTTP requests
  are printed. Gradle's cache is ours (`actions/cache` restore / save, key
  `gradle-<OS>-<hash of the Gradle files>`, falls back to the latest
  entry), saved only after a successful build on `main`: setup-gradle's
  basic provider (its cache now off) saved a failed run's stub for good.
  The Android version lives in `app/version.properties`, outside the key
  (#23); the release APK job restores the same key, read-only.
  Portal-only plugins (foojay, ktlint) are resolved from
  the plugin portal only (`exclusiveContent` in `settings.gradle.kts`).
- **License**: GPL-3.0-or-later (`LICENSE`, `license` in the workspace
  `Cargo.toml`, `package.json`, `tauri.conf.json`). Third-party licenses must
  be GPL-compatible and are enforced at build time: Rust by cargo-deny
  (`deny.toml`) and cargo-about (`about.toml`, same allow-list), npm by
  `scripts/licenses.mjs` (`NPM_ALLOWED`), Android by the Licensee Gradle
  plugin (`licensee { allow(...) }`). Adding a dependency under a new license
  means extending these lists (after checking compatibility).
- **"Licences" screens**: desktop connections view → "À propos" card →
  dialog fed by `apps/desktop/src/generated/licenses.json`, built by
  `scripts/licenses.sh` (not committed; CI and releases build it, dev builds
  show an empty list). Tablet: "À propos et licences" (setup screen and
  connection menu), from Licensee's `assets/licenses/libraries.json` (built
  with the APK) plus the texts in `assets/licenses/`.
- **App icon**: single source `assets/icon/padboard.svg` (the 2×2 grid, the
  bottom-right tile being pressed); `node scripts/icons.mjs` generates every
  file from it (desktop PNGs, multi-size `.ico`, tray PNGs, favicon (also the
  editor's header badge), Android adaptive + monochrome vector drawables,
  `ic_padboard` for the tablet's headers, the 512 px store listing icon in
  `fastlane/`; the adaptive icon is Android 8+ (`mipmap-anydpi-v26`),
  Android 7.x gets PNG launcher icons, square and round). 32 px and below drop the halo
  (blurry under a pixel). Never edit the generated icons by hand.
- **Releases** (`docs/releasing.md`): one SemVer version for everything,
  `-rc.N` for release candidates. `scripts/release.sh X` writes it everywhere
  (Android versionCode = MAJOR*1e6 + MINOR*1e4 + PATCH*100 + rc number, 99
  for the release, in `apps/android/app/version.properties`), regenerates `CHANGELOG.md` (git-cliff, `cliff.toml`:
  feat/fix/perf only; 0.1.0 has a written summary instead), commits and tags without pushing. A pushed `v*` tag
  runs `release.yml`: NSIS installer (unsigned: SignPath Foundation declined
  the request for now, may be asked again later; SmartScreen warning
  expected; VirusTotal checked before publishing, also by `verify-release`
  with `VIRUSTOTAL_API_KEY`; reported to Bitdefender or Microsoft only
  when one detects it (`STOP:`) or a user reports a block, no longer
  systematically since 0.2.2; other engines only after 2 weeks), .deb
  (built on Ubuntu 22.04; an rc's package version gets a tilde,
  `0.2.1~rc.1`, by `scripts/deb-version.py`, so Debian sorts it before
  the release; file name unchanged), unsigned APK, draft release (pre-release for rc)
  with SHA256SUMS. `scripts/sign-release.sh X` then signs the draft on the
  maintainer's PC (keys never go to CI, `docs/release-signing.md`): the
  APK (certificate checked), the installer for the updater (minisign,
  `--app-version`: the version is in the signed trusted comment),
  `latest.json`, and `SHA256SUMS` written again and signed for X; checked
  by the `verify-release` example (the app's own code) before upload.
  If Authenticode signing (SignPath) comes one day, it signs the
  installer in CI before the draft, so the minisign signature covers the
  final file; the script refuses an unsigned installer once the workflow
  has a SignPath step.
  Public update key: `apps/desktop/src-tauri/updater-key.pub` (empty =
  updates off; the release workflow refuses it).
- **Desktop updates** (`updates/`): the releases list from the GitHub
  API (`Hugoambiehl/Padboard`), at launch + 10 s then daily, on by
  default, pre-releases only with "Versions de test" (`preferences.json`
  `updates`); never in test mode, off when `updater-key.pub` is empty.
  An automatic check that cannot reach GitHub shows nothing (logged
  only) and is retried after 5 then 30 min, then daily (`next_check`);
  "Rechercher" shows its failures.
  **Never start an unsigned Windows installer automatically**: started
  by Padboard rc.1, the unsigned rc.2 installer was taken by Bitdefender
  ATC (`Atc4.Detection`) and Padboard itself quarantined, with no message
  (`windows-test-plan.md` 8.5). Windows ("Télécharger",
  `updates/installer.rs`): our own code downloads the installer and its
  `.sig` (minisign, bound to file name and version) and the signed
  `SHA256SUMS`, saves it in Downloads (test mode: `<test dir>/config/
  downloads`), then `installer::decide`: started only if WinVerifyTrust
  validates an Authenticode signature by `TRUSTED_PUBLISHER` (empty: no
  certificate, SignPath declined for now, so never; if one comes, check
  the issuer too, not only the name), else `UpdateStatus::Ready`: the Explorer opens on the
  file selected (`explorer /select`), the card says « Ferme Padboard, puis
  lance ce fichier » with *Afficher le fichier*, *Quitter Padboard* and
  the release page. The trusted path goes through `tauri-plugin-updater`
  ~2.12 (last for tauri 2.11 / Rust 1.88, Windows-only dependency, driven
  from Rust only: no JS package, no permission for the page) with our
  checked bytes, NSIS wizard (`basicUi`; `passive` only with a signed
  installer), pending edits flushed in `on_before_exit`. The version and file
  are written to `update-pending.json`; a launch still on the older
  version offers the file again (`Ready`), or shows `NotInstalled` if it
  is gone ("Que faire ?" opens `help.rs`'s page). An automatic check
  never replaces a failed or ready install of the same version with
  "available" (only "Rechercher" offers it again). Linux: notice +
  release page (opened through the `open` action, URL built by the
  backend); test sources (`test-updates`) also take the Windows path on
  Linux, for `e2e/updates` (the e2e build has the feature; nothing is
  shown outside Windows). Our HTTP: `reqwest` with `rustls-no-provider`
  (ring installed as default provider). UI: "Mises à jour" section of
  the About card, header badge; `types/updates.ts` mirrors `UpdatesView`
  by hand. The Windows-only code cannot be cross-checked from Linux (ring
  needs MSVC's `lib.exe`): CI's Windows job builds it.
- **Tablet updates from the PC**: always to the PC's own version (the
  APK of the same release; a development version, outside the
  `versionCode` scheme, offers nothing). `updates/tablet.rs` downloads
  `SHA256SUMS`, its `.sig` and the APK into `<config>/updates/`, checks
  the signature (bound to the version) and the hash every time it is used,
  keeps only the current version. USB (any tablet with Padboard, kiosk or
  not, any protocol): the version is read with `dumpsys package`
  (`UsbDevice.app`), then `adb install -r` (never `-d`), 180 s timeout,
  Android's refusals mapped to codes (`UsbUpdateError`). Wi-Fi: kiosk
  tablets only, over the connection (protocol 5 core): the tablet
  (`ConnectionManager`, JVM-tested against the fake PC) refuses outside
  device-owner mode or for a version not above its own, pulls the pieces
  one at a time (30 s stall limit, file deleted on any failure), checks
  the SHA-256, then `AppUpdater.check` (`ApkChecks`: same package, higher
  `versionCode`, exactly its own signing certificates, and in a release
  build — not debuggable — the release certificate only), reports
  `installing`, then commits a silent `PackageInstaller` session
  (`update/AndroidUpdater.kt`; status in `InstallStatusReceiver`: a
  confirmation request is never shown, it means the app is no longer the
  device owner). An update-only session sends no `tablet:status` and no
  presses, and shows `UpdateOnlyScreen` (which side to update). The device owner
  installs without `REQUEST_INSTALL_PACKAGES` (AOSP
  `computeUserActionRequirement` waives it, 8.0 and 12L checked): the
  permission is not declared.
- **Development version**: `main` always carries `X.Y.Z-dev` between
  releases (versionCode `…·100 + 0`, below rc.1); `release.sh` adds a
  `chore: start X.Y.Z-dev` commit after the tagged release commit;
  `scripts/check-version.sh` (CI) refuses a released version or files
  that disagree; `scripts/dev-version.sh` sets it by hand
  (`scripts/lib/set-version.sh` writes every file). A development PC
  offers no tablet update (`version_code` is `None`); the USB card and the
  connected tablets card explain instead (`ServerStatus.development`,
  `OutdatedTablet` now also for USB). `desktop-dev.sh` warns when it runs
  on the real configuration.
- **Tray and startup** (`tray.rs`): the icon's menu opens the window,
  shows the connected tablets, quits. Closing the window hides it there
  (explained once by a dialog, `closeHintShown` in `preferences.json`);
  on Linux without a StatusNotifier host (GNOME without the AppIndicator
  extension) closing quits, so the app never runs unreachable. A second
  launch shows the running window (single-instance plugin, not in test
  mode). "Démarrage" card: start with the session (`autostart.rs`: the
  `auto-launch` crate with the program path quoted by us, since neither it
  nor the Tauri plugin quote it; entry with `--hidden`, off by default,
  rewritten at launch when on; simulated in test mode). The choice is
  kept in `preferences.json` (`autostart`; absent = recorded from the
  entry at the next launch): a `Run` value gone while it is on (Bitdefender
  ATC remediation, Tauri's uninstaller) is written again at launch and
  logged, the value only, never `StartupApproved` (auto-launch's `enable`
  marks it enabled), so a Task Manager "Disabled" stays; on Linux a
  missing `.desktop` file turns the choice off (GNOME removes it). The log file is
  opened in setup, after the single instance check (a second launch must
  not truncate the running instance's log). The tooltip the icon is
  created with names it in Windows settings: fixed "Padboard", the status
  is set right after. Windows menus are forced dark (uxtheme
  `SetPreferredAppMode`). e2e tests quit
  with `destroy` (closing only hides). The `.deb` build needs
  `libayatana-appindicator3-dev` (the bundler then adds the runtime
  dependency itself).
- **Secret scanning**: gitleaks on the whole history in CI (`.gitleaks.toml`;
  the fixtures' fake token is allowed by value and path, not by commit, so
  the exception survives a history rewrite); GitHub secret scanning with
  push protection, CodeQL default setup (Rust, JS/TS, Actions; Kotlin is
  not analyzed: its autobuild looks for Gradle at the root), Dependabot
  (monthly), private vulnerability reporting.
- **Secrets never printed**: types holding a secret (token, PIN, OBS
  password) implement `Debug` / `toString()` by hand with
  `padboard_protocol::REDACTED`, tested; a new secret field gets the same.
  The private TLS key is already elided by rcgen / rustls. The generated
  Kotlin protocol classes still print theirs: never log a message whole.
- **Store listing** (IzzyOnDroid): fastlane metadata in
  `fastlane/metadata/android/{en-US,fr-FR}` (descriptions, tablet
  screenshots, a changelog per release versionCode, see
  `docs/releasing.md`). No dependency info block in APKs
  (`dependenciesInfo` off).

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
- **Edited profile ≠ displayed profile**: the configuration's
  `activeProfileId` is the profile the tablets show, changed only by
  "Afficher sur la tablette" (`SetActiveProfile`). The profile open in the
  editor is local frontend state (`useEditor().editedProfile`, falls back to
  the displayed one); the profile menu only changes it, and marks the
  displayed profile. Deleting the displayed profile moves the tablets to the
  first remaining one (the confirmation names it).
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
- **Action forms** (`ActionForm.vue`, recursive for multi action steps): a
  choice in a list ends the undo entry (`checkpoint`), typing merges into
  one (`SetAction` and `SetActiveLook` have a merge key). OBS scenes and
  inputs come from OBS when connected, else are typed. Stateful buttons
  (microphone, output mute, OBS) have an optional `activeLook` (label, icon,
  color over the automatic active look). Native selects get a chevron (they
  lose the GTK look). In e2e tests, pick options by setting the value and
  dispatching `change`: WebKitWebDriver does not fire it on an option click.
- **Lucide upgrades**: run `node scripts/export-icons.mts` then the ignored
  Rust test `all_lucide_icons_are_valid` (see `svg_path.rs`), so every icon
  still passes the desktop's path validator.

## End-to-end tests (desktop)

- `scripts/desktop-e2e.sh` (local only, never in CI): tauri-driver +
  WebKitWebDriver drive the real window; tests in `apps/desktop/e2e/*.e2e.ts`
  (Vitest, `e2e/vitest.config.ts`), through a small hand-written W3C WebDriver
  client (`e2e/webdriver.ts`, no WebdriverIO). Texts come from `fr.json` via
  `t()`, so wording changes do not break selectors.
- **Test mode** `PADBOARD_TEST_DIR` (`src-tauri/src/test_mode.rs`): separate
  config dir, `RecordingExecutor` (actions written to `actions.jsonl`, never
  injected), no adb, Wi-Fi forced off (except `PADBOARD_TEST_SYSTEM=1` with
  `PADBOARD_BIND=lan`), no tray icon (it sat next to the user's own
  Padboard and was taken for it) unless `PADBOARD_TEST_TRAY=1` (e2e/tray),
  then named "Padboard (test)"; without it closing the window quits,
  logs to `app.log` (normal mode: `padboard.log` in the
  config dir, started afresh at launch, the previous one kept as
  `padboard.log.1`), and a runtime
  simulated button states from `state.json` (`observe::start_from_file`),
  and a runtime capability letting the page close its window (WebDriver kills the process
  when it ends a session, so "close the app" goes through the page).
- **Tests on the real tablet and sound devices**: test mode with
  `PADBOARD_TEST_SYSTEM=1` (configuration still isolated, actions really run,
  states really observed, adb used: the tablet's 47321 is redirected to the
  test app's port, give it back with `adb reverse tcp:47321 tcp:47321`); drive the tablet with `adb shell input tap/swipe`
  and check `adb exec-out screencap -p`; change the system with `wpctl`
  (`pactl` is not installed); `--example fake-player` (MPRIS player on the
  session bus) for media; save and restore the exact audio state around
  each test with `--example audio-snapshot -- save|restore <file>`. Never
  press hotkey buttons in system mode (real keystrokes on the PC). Kill test
  processes with `pkill -x <name>`, never `pkill -f` (it matches the shell
  running the command).
- **AppArmor and snaps**: processes started from the VS Code snap terminal
  carry its AppArmor label, and snap players (Firefox) refuse their MPRIS
  calls: media commands still work (media-key fallback) but the playing
  state is not visible. The app launched from GNOME is unconfined. For
  tests that need it, start the app with `systemd-run --user --unit=…
  --setenv=…` (unconfined); same for `--example state`.
- Steps of a file build on each other (`bail: 1`); a failed step saves a
  screenshot `FAILED <step>.png`. Review the screenshots after UI changes.
- The app quits cleanly on SIGTERM/SIGINT/SIGHUP (Ctrl+C on Windows), so
  pending edits are saved on logout too.

## Conventions

- **Paths contain a space** (`Projets persos`): always quote paths in shell scripts,
  CI and config.
- Rust: `cargo fmt` + `cargo clippy -- -D warnings`. TS: ESLint + Prettier.
- **Build size** (the dev PC is short on disk): dev profile with line
  tables only for our crates and no debug info for dependencies; the e2e
  build (`target/e2e`, separate so `tauri dev` stays cached) runs without
  incremental cache or debug info. Dev + e2e ≈ 5.5 GB (was 60 GB).
  Don't build twice without need (`desktop-e2e.sh --no-build` reuses the
  last build); clean temporary files at the end of each step.
  Kotlin: ktlint.
- Tests: protocol round-trip serialization, configuration logic (migrations…),
  frontend logic (Vitest), end-to-end on the real window (local, see above).
- Config persisted as versioned JSON (with migrations) in the user config dir.
- Commits: small, **in English**, Conventional Commits
  (`feat|fix|docs|style|refactor|test|chore|perf|build|ci(scope): summary`). No
  `Co-Authored-By` / "Generated with" lines.
- Never commit secrets, keystores or `local.properties`.
- Work phase by phase; ask before any significant architecture decision.

## Roadmap

1. **Foundations** ✅ — workspace, protocol + typeshare, minimal WS server in Tauri
   running a keyboard shortcut (testable on Linux), Android app connecting by manual
   IP and showing a fixed grid.
2. **Connection** ✅ — mDNS + PIN pairing, USB via adb reverse, auto-switch,
   reconnection, heartbeat.
3. **Editor** ✅ — replica, library, inspector, drag & drop, persistence, live
   sync, undo/redo, edited vs displayed profile; tablet icons and page swipes;
   end-to-end tests on the real window (local).
4. **Actions** ✅ — all MVP actions (Linux + Windows), stateful buttons, folders,
   pages, multi-actions.
5. **Tablet polish** ✅ — kiosk mode (device owner, launcher, lock task),
   disconnected overlay, screensaver, animations, performance. Checked on the
   tablet: reboot (starts alone, locked, reconnects), screensaver clock
   drifting over 13 min.
   Backlog: QR code pairing (the fingerprint comparison is enough for now);
   dark window title bar on GNOME (`theme: "Dark"` is set, Hugo checks it);
   user images as icons.
6. **Distribution** — Windows installer, signed release APK, CI, README, open source.
   License notices to ship: Lucide icons (ISC), Inter and Space Grotesk (OFL).
