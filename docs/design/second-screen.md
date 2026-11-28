# Second screen (#20): design

Status: **reviewed by Hugo on 2026-11-27**, decisions in section 10.
Target: 0.5. Inputs: issue #20 (idea, license review, decision of
2026-11-27) and the throwaway prototype on branch `proto/mirror` (step 1,
Linux → SM-T580 over USB, measured 2026-11-26).

## 1. What the user gets

A new kind of button, **"Second écran"**. Pressing it on the tablet turns
the tablet into a display of the PC; touches act as the mouse. A long
press on a volume key of the tablet (or stopping it from the PC) brings the
grid back. One install only: everything is in Padboard, or downloaded by
Padboard on demand (the Windows virtual display driver), free and open
source.

## 2. Order of delivery, order of construction, releases

### Delivery

Each step is usable on its own and ships behind the same button.

| Step | What | Needs |
|---|---|---|
| 1 | **Mirror on Windows**, no driver: the tablet shows one of the PC's monitors | Capture + encoder on Windows, stream, tablet screen |
| 2 | **Virtual screen on Linux**: an extra monitor through the ScreenCast portal's `VIRTUAL` source (GNOME: `AvailableSourceTypes` = 7, no driver) | Portal session, GStreamer pipeline |
| 3 | **Touch as mouse**, both systems | Input back from the tablet; injection on each OS |
| 4 | **Extended screen on Windows** with Virtual Display Driver, downloaded and installed on demand | Download + checks + elevated install, monitor on/off |

The Linux mirror (a monitor that exists) comes for free with step 2 (the
same portal, `MONITOR` source), and the Windows mirror stays available
after step 4 for users who refuse the driver.

### Construction

The order of delivery does not change, but the code is built from the
common parts outwards, so that everything except the Windows capture is
tested on the development PC first:

1. **Common plumbing**: protocol (capability, offer, stop), the stream
   connection and its ticket, flow control, the USB peer check
   (section 4), the tablet's decoder and screen. Tested with a **test
   pattern source** on the PC (moving bars, a frame counter, a clock:
   latency readable on a photo), then with the **Linux source**
   (portal + GStreamer) on this PC and the SM-T580.
2. **Windows capture** (Desktop Duplication + Media Foundation), plugged
   into the same plumbing: step 1 for the user.
3. Linux virtual screen (step 2), then touch as mouse (step 3).
4. VDD (step 4), only after the checks on Windows (section 7).

### Releases

- **0.5.0-rc.1**: steps 1 to 3.
- **A later 0.5 rc**: step 4 (VDD), after Hugo's checks on Windows.

## 3. Capture and encoding

Common target, from the prototype: **H.264, 1728×1080, 60 fps, CBR
8–12 Mbit/s, no B-frames, a key frame every 2 s**, constrained baseline or
main profile, level ≤ 4.2. 1728×1080 keeps the tablet's 16:10 shape under
the decoder limit (section 5). A PC monitor of another shape (16:9) is
scaled to fit with bars; the virtual monitors (steps 2 and 4) are created
at 1728×1080, so no bars.

### Windows

- **Capture**: Desktop Duplication (`IDXGIOutputDuplication`, DXGI 1.2,
  Windows 8+). It gives a D3D11 texture per change, the dirty rectangles
  and the cursor shape separately; it works for any output, including an
  IddCx virtual monitor (step 4). Windows.Graphics.Capture is the
  alternative (yellow border on Windows 10 unless 2104+, needs WinRT
  activation); Desktop Duplication first, WGC only if Duplication fails
  on some GPU (hybrid laptops: duplication must run on the adapter that
  owns the output).
- **Scale + colour conversion**: on the GPU, D3D11 video processor
  (`ID3D11VideoProcessor`) BGRA → NV12 at the target size.
- **Encoder**: Media Foundation's H.264 encoder MFTs, part of Windows:
  the GPU vendor's hardware MFT (NVENC, Quick Sync, AMF) when present,
  else Microsoft's software H.264 encoder. Low-latency mode
  (`CODECAPI_AVLowLatencyMode`), CBR, GOP 120, no B-frames. Hardware MFTs
  are asynchronous (event-driven): one encoder thread.
- **Cursor**: composited into the frame by us (Duplication gives it apart).
- Everything through the `windows` crate already in use (MIT/Apache-2.0).
  Patents: the H.264 encoders are licensed with Windows.

### Linux

- **Capture**: xdg-desktop-portal **ScreenCast** (PipeWire), as in the
  prototype; `VIRTUAL` source for step 2, cursor embedded, persist mode 2
  with a restore token so the consent dialog shows once.
- **Encoder**: **GStreamer** through `gstreamer-rs` (MIT/Apache-2.0
  bindings, the system's libgstreamer, LGPL-2.1+):
  `pipewiresrc ! vapostproc ! vah264enc` (VA-API, measured 23.6 ms p50,
  low CPU). Fallback when VA-API H.264 encoding is missing (some drivers
  or distributions build it out): **VP8** with `vp8enc` (libvpx,
  BSD, royalty-free; measured 31.1 ms p50, ~1.2 cores), decoded in
  hardware on the SM-T580. `x264enc` (GPL-2.0+, patent-encumbered, in
  plugins-ugly) is not used.
- The `.deb` depends on `gstreamer1.0-pipewire`, `-plugins-base` and
  `-plugins-good` (VP8), all on an Ubuntu desktop already, and only
  recommends `gstreamer1.0-plugins-extra` (the `va` plugin on Ubuntu
  26.04+; `-bad` before, +121–144 MiB, never a dependency). Checked at
  run time: a missing element gives a clear message, never a crash.
- **Built** (`screen/portal.rs`, `zbus` already used by the desktop):
  `CreateSession`, `SelectSources` (`VIRTUAL`, cursor embedded, persist
  mode 2), `Start`, `OpenPipeWireRemote`, each answer awaited on its
  `Request` (watched before the call, 2 min for the user to answer).
  The restore token is kept owner-only in `screen-cast-token` (each one
  is good once: the new one replaces it). The virtual monitor takes the
  stream's size from the caps after `pipewiresrc` (1728×1080 for the
  SM-T580, `max-framerate` 60). Starting runs on a blocking thread
  outside the sessions lock, in its own task; a second press while the
  dialog is open is ignored. Available with
  `PADBOARD_SCREEN_SOURCE=screen` until the tablet shows it.
- Wayland and X11 GNOME, KDE Plasma: the portal works on all three; the
  `VIRTUAL` source is GNOME (mutter) and KDE 6 only. Elsewhere: mirror
  only, said so.

### Test pattern source

A third source, in the desktop crate, for construction and tests: frames
drawn by us (bars moving one step per frame, frame number, PC clock in
milliseconds), encoded by the same encoder as the Linux source. It needs
no portal and no consent, so it also runs in test mode and in e2e tests.

### Damage-driven frames

Both systems send a frame only when the screen changes (GNOME: ~10 fps
while scrolling by hand in the prototype). The SM-T580's decoder keeps
one frame inside, so the **last change would stay invisible until the next
one**. The PC therefore repeats the last captured frame (encoded again,
tiny P-frame) one frame time after the last change, once. A static screen
then costs nothing.

Built with `pipewiresrc keepalive-time=500`: the last picture is sent
again every 500 ms while the screen is still (also what lets a key frame
asked for on a still screen arrive). Faster repeat, once, after a change:
to decide with the tablet (remaining 4).

## 4. Transport and who may connect

### Choice: a second WebSocket, same port, same TLS

| Option | For | Against |
|---|---|---|
| A. Inside the existing session (binary messages) | Nothing new to open | Head-of-line blocking: pings, button states and presses wait behind video; one outbox for both |
| **B. A second WebSocket to the same port, path `/stream`** | Same `adb reverse` (USB) and same TLS + pinned key (Wi-Fi); no new firewall rule; the control session stays light | A ticket to tie it to the session |
| C. Raw TCP on another port | Least overhead | Another `adb reverse`, another TLS setup, another Windows firewall prompt |

**B.** The tablet opens it only when the PC offers a stream, with a
**one-time ticket** (32 random bytes, valid 10 s, bound to the session
and its device) received on the authenticated session. The server routes
by path in the handshake callback it already has (`accept_hdr_async`,
Origin check unchanged). Nothing in the frozen core changes: the stream
connection never sends `hello`.

### USB: check the process on the other end

USB sessions are trusted without PIN because the cable is the proof. But
`hello` only *declares* who is connecting, so a confirmation "per
device" proves nothing: any local process can reach `127.0.0.1:47321`
and claim to be the tablet. A real USB connection, though, always comes
from **the adb server**: `adb reverse` makes the adb server open the
connection on the PC side. So the desktop checks, when it accepts a
connection on the USB port, which process owns the other end:

- **Linux**: the accepted socket's peer address (`127.0.0.1:<port>`) is
  looked up in `/proc/net/tcp` (and `tcp6`): the row whose local address
  is that peer and whose remote port is ours gives the socket's inode and
  uid. The process owning that inode (`/proc/<pid>/fd/* →
  socket:[inode]`) must be the process listening on the adb server port
  (5037, or `ANDROID_ADB_SERVER_PORT`, the same lookup with state
  `LISTEN`). A server run by another account (root through `sudo adb
  start-server`) hides its descriptors: then the peer socket's uid must
  be the listener's uid. Checked on this PC on 2026-11-27: the tablet's
  session socket belongs to `/usr/lib/android-sdk/platform-tools/adb`,
  uid 1000, the process listening on 5037.
- **Windows**: `GetExtendedTcpTable` with `TCP_TABLE_OWNER_PID_CONNECTIONS`
  gives the owning PID of the client side of the connection (no
  administrator rights needed, like `netstat -ano`); it must be the PID
  listening on 5037 (`server_owner.rs` already finds it).
- The check runs **once per connection**, at accept, before `hello` is
  read; a refusal closes the connection and is logged with the peer's
  program (when known) and shown in the USB card ("une connexion USB
  refusée : elle ne venait pas d'adb").

**Applied to the whole USB port, keys included** (recommended): another
account sending keys into the session is as serious as reading the
screen, and the check costs nothing for a real tablet. What it changes:

| Who connects to the USB port | Today | With the check |
|---|---|---|
| The tablet through `adb reverse` | Accepted | Accepted |
| A program of another account | Accepted | **Refused** |
| A sandboxed app of the same account (snap, Flatpak with network) | Accepted | **Refused** |
| `fake-tablet`, e2e tests, `server.rs` tests | Accepted | Accepted in **test mode** only (`PADBOARD_TEST_DIR`) and in unit tests (the check is a `PeerPolicy` in `ServerContext`: `AdbServer` normally, `Any` there) |
| An emulator on another adb server (`ANDROID_ADB_SERVER_PORT=5039`, our dev setup) | Accepted | Refused unless Padboard uses that server too: the check follows `ANDROID_ADB_SERVER_PORT` like adb does |

What it does **not** stop, written in SECURITY.md: the adb server itself
accepts commands from any local account on port 5037 (adb's own model),
so another account can still run `adb shell` on the tablet and open a
connection from the tablet's side, which then arrives through the adb
server. The check removes the direct path (connect to a port); it does
not make a shared PC safe against its other accounts. An app on the
tablet reaching the tablet's `127.0.0.1:47321` also arrives through adb:
the tablet is a dedicated device, kiosk mode allows no other app.

### Starting a stream: confirmation and indicator

- **Confirmation on the PC the first time** a device starts a second
  screen: a dialog in Padboard's window (brought to the front) naming the
  device and the transport, "Autoriser" / "Refuser". It is **remembered
  only when the connection is verified**: a Wi-Fi session (paired, TLS,
  pinned key, token) or a USB session that passed the peer check. In test
  mode the peer check is off, so nothing is remembered there.
- **Always visible while it runs**: the tray icon changes (a variant of
  the Padboard icon with a dot, generated by `icons.mjs`) and its tooltip
  says "Second écran : <device>"; the tray menu gains **"Arrêter le second
  écran"**; the window's header shows the same with an "Arrêter" button.
  On GNOME the portal adds its own screen sharing indicator; on Windows
  ours is the only one, so it is never hidden.

### Frames and flow control

- PC → tablet, binary messages: `u8 kind` (frame), `u64` capture time
  (µs, PC monotonic clock), `u8 flags` (key frame), then the access unit
  (H.264 Annex B, SPS/PPS repeated before key frames).
- Tablet → PC, small binary messages: **ack** (capture time, decode µs)
  when a frame is released to the display, as in the prototype; **input**
  (section 6); **keyframe request** after a decoder error.
- The PC allows **at most 2 frames sent and not yet acknowledged**. Above
  that, captured frames are dropped *before* the encoder (never an
  encoded P-frame, which would break the picture), so a slow link lowers
  the frame rate instead of piling up latency. Over Wi-Fi the bitrate
  steps down (12 → 8 → 5 → 3 Mbit/s) when frames are dropped for more
  than a second, and back up slowly.
- `TCP_NODELAY` on both ends (OkHttp sets it; tungstenite: on the socket).

### Targets

| | USB (`adb reverse`) | Wi-Fi (TLS) |
|---|---|---|
| Bitrate | 12 Mbit/s (40 passed without loss in the prototype) | 8 Mbit/s, adaptive down to 3 |
| Frame rate | 60 | 60, lower under load |
| Latency, capture → shown (p50) | ≤ 30 ms (prototype: 23.6 ms) | ≤ 50 ms |
| Glass to glass (estimate) | ~40–55 ms | ~60–80 ms |

Wi-Fi was not measured yet; the reachability limits of
`install-tablet.md` (client isolation, meshes) apply even more here.

## 5. Tablet

- **Decoding**: `MediaCodec` onto a `SurfaceView` (hardware composition,
  no Compose drawing per frame). SM-T580: `OMX.Exynos.avc.dec` (level
  4.2, **max 1920×1088**: no native 1920×1200; the software decoder
  manages ~15 fps) and `OMX.Exynos.vp8.dec`; `KEY_LOW_LATENCY` refused
  (-1010): configured without it after a refusal (prototype). The tablet
  sends its decoders' limits (`MediaCodecInfo`) in the capability, the
  PC picks the size (1728×1080 for the SM-T580).
- **The second screen**: a full-screen `SecondScreen` composable hosting
  the `SurfaceView` (`AndroidView`), black bars when the picture is not
  16:10, screen kept on, screensaver off while it runs. While it starts:
  "Connexion au PC…"; if the stream stops (PC stopped, link lost): back
  to the grid, with the usual disconnected card if the session is gone.
- **Leaving it** (decided, section 10):
  - **a long press on a volume key** (either): the activity receives the
    key events even in lock task mode; the second screen tracks the
    press (`KeyEvent.startTracking`, `onKeyLongPress`) and consumes both
    volume keys while it runs (no system volume panel over the picture);
  - **"Arrêter le second écran" on the PC** (tray menu, window);
  - maybe a **tab at the top centre**, faded (alpha only), that reacts
    only to a press held 1 s, never to a tap, so a click near the top of
    the PC's screen is not taken. Tried on the tablet before it is kept.
  - Not the top-right corner: on Windows it is the close button of
    maximised windows.
- **Kiosk corner**: in kiosk mode the bottom-left 40 dp stay free as on
  every screen. The picture is still drawn there, but touches that
  **start** in the corner are not sent to the PC, so the 5 s exit gesture
  works and never holds the mouse down on the PC. That area of the PC's
  picture (about 54×54 pixels of the 1728×1080 picture, 60×60 on a
  1920×1080 monitor: Windows 10's Start button,
  the left end of a bottom taskbar) cannot be touched; documented in
  `install-tablet.md` (use a grid button for the Windows key instead).
  `KioskCornerTest` covers the new screen.
- **Memory**: decoder buffers ~6 × 2.8 MB. Dropping the grid's cached
  page layers while the stream runs would save most of the ~65 MB they
  take, but the first composition of the grid costs ≈ 1.3 s of main
  thread on the SM-T580 (CLAUDE.md, tablet startup). **Measured before
  deciding**: the return to the grid with and without the layers
  (release build, `dumpsys gfxinfo` framestats, frame times of the first
  second). The layers are kept if dropping them makes the return
  stutter; memory is then checked with the stream running (`dumpsys
  meminfo`, no low-memory kill of the app).

## 6. Touch as mouse (step 3)

Absolute positions (the cursor goes where the finger is), normalised to
the picture (0–1), so the PC maps them to the right monitor:

| Touch | Mouse |
|---|---|
| Tap | Left click |
| Hold still 500 ms | Right click |
| Drag | Left button down, move, up |
| Two-finger drag | Scroll (wheel / smooth scroll) |
| Pinch | Nothing in 0.5 |

- **Windows**: `SendInput` with `MOUSEEVENTF_ABSOLUTE |
  MOUSEEVENTF_VIRTUALDESK`, coordinates mapped to the captured monitor's
  rectangle in the virtual desktop. Touch injection
  (`InjectSyntheticPointerInput`, Windows 10 1809+) is a later option.
  Elevated windows ignore it (known limit, already documented for keys).
- **Linux**: the **RemoteDesktop portal**, in the same session as
  ScreenCast (one consent dialog): `NotifyPointerMotionAbsolute` on the
  stream, `NotifyPointerButton`, `NotifyPointerAxis`. `uinput` cannot aim
  at one monitor on Wayland, so it is not used here.
- Input travels on the stream connection: it ends with the stream.

## 7. Windows extended screen (step 4): Virtual Display Driver

Decided by Hugo (#20, 2026-11-27; `CLAUDE.md`, exceptions): downloaded on
demand, never bundled. **No VDD code before the checks below, made by
Hugo on Windows.**

- **Download**: the official signed release of
  `VirtualDrivers/Virtual-Display-Driver` on GitHub, version and SHA-256
  pinned in the code (bumped by a script, like
  `platform-tools-pin.sh`); each `.dll`, `.cat`, `.sys` checked for a
  valid Authenticode signature by the SignPath Foundation (signer **and**
  issuer). The latest release (25.7.23, 2025-07-23) ships
  `VirtualDisplayDriver-x86.Driver.Only.zip`; the x64 "driver only"
  package to pin is to be identified at that time (25.5.2 had
  `Signed-Driver-v24.12.24-x64.zip`).
- **Dialog first**: what it is (a virtual monitor driver, a separate
  project), MIT and MS-PL, not made by Padboard, an administrator prompt
  follows, how to remove it.
- **Install**: Padboard starts itself elevated once (`runas`,
  `padboard-desktop.exe --install-display-driver <dir>`): `pnputil
  /add-driver MttVDD.inf /install` and a root-enumerated device created
  with SetupAPI (`SetupDiCreateDeviceInfo` + `DIF_REGISTERDEVICE` +
  `UpdateDriverForPlugAndPlayDevices`). Removal from Padboard: device
  removed, `pnputil /delete-driver`.
- **Mode list** written to VDD's settings (1728×1080 @ 60 first).

### Turning the monitor on and off

The virtual monitor should exist only while the second screen runs
(Windows moves windows onto it otherwise).

**VDD's control pipe** (read in `MttVDD/Driver.cpp`, master, 2026-11-27):

- `\\.\pipe\MTTVirtualDisplayPipe`, message mode, created by the driver
  (in its WUDF host) with the security descriptor `D:(A;;GA;;;WD)`:
  **Everyone, full access**, no integrity label beyond the default
  (medium): any process of any account at medium integrity can use it,
  **no administrator rights**.
- Commands (UTF-16 text, one per connection, 127 characters max):
  `PING` (answers `PONG`), `SETDISPLAYCOUNT <n>` (writes the count to
  `vdd_settings.xml`, then `RELOAD_DRIVER`), `RELOAD_DRIVER`
  (re-initialises the adapter), `GETSETTINGS`, `SETGPU "<name>"`,
  `GETALLGPUS`, `GETASSIGNEDGPU`, `D3DDEVICEGPU`, `IDDCXVERSION`, and
  toggles (`LOGGING`, `LOG_DEBUG`, `HDRPLUS`, `SDR10`, `CUSTOMEDID`,
  `PREVENTSPOOF`, `CEAOVERRIDE`, `HARDWARECURSOR` with `true`/`false`).
- Settings live in `C:\VirtualDisplayDriver\vdd_settings.xml` (path
  from the registry), written by the driver itself.

So `SETDISPLAYCOUNT` could turn the monitor on and off without
elevation. To check on Windows before any code:

1. Does `SETDISPLAYCOUNT 0` work (the adapter is initialised with
   `MaxMonitorsSupported = numVirtualDisplays`: 0 may fail), and does a
   reload really unplug the existing monitor (no `IddCxMonitorDeparture`
   seen in the reload path) and plug it back with `SETDISPLAYCOUNT 1`?
2. How long a reload takes, and whether windows come back to the main
   monitor.
3. The pipe's real ACL (`accesschk` or PowerShell `[System.IO.Directory]::GetAccessControl`
   on the pipe) and whether a standard user can use it.
4. Windows 10 and 11, Secure Boot on; Bitdefender's reaction to the
   driver install and to the pipe use.

**Security note** (for the dialog and SECURITY.md): the pipe is VDD's,
present whenever it is installed. Any local account can change its
settings or reload it; a malformed `SETGPU` (no quotes) reads past the
string it builds and may crash the driver host. That is a property of
VDD, said in the dialog ("a separate project with its own control
channel"), and a reason to offer its removal from Padboard.

**If the pipe cannot do it**: **an administrator prompt at each start**
of the second screen (disable / enable the device through an elevated
Padboard), rather than a monitor plugged in for good. **No scheduled
task running elevated** to avoid the prompt: Bitdefender ATC flags that
pattern.

## 8. Protocol changes (`crates/protocol`)

- **Protocol 5 stays** (decided): `hello` gains an optional `screen`
  capability (frozen core: only an optional field): codecs, max
  width/height per codec, whether touch is sent. Tablets without it
  never see a second screen button work (the PC answers the press with a
  clear error on the tablet). A 0.4 tablet keeps working with a 0.5 PC.
- New messages on the session (names settled when coding, round-trip
  tests and fixtures as usual):
  - PC → tablet `screen:offer` { ticket, codec, width, height, fps };
  - tablet → PC `screen:stop` (volume key long press);
  - PC → tablet `screen:stopped` { reason } (PC stopped, capture lost,
    consent refused on Linux, refused on the PC…).
- The stream connection's binary format (section 4), versioned by a
  first byte, documented next to the types, with fixtures tested in Rust
  and Kotlin like the others.
- New action `SecondScreen` (`crates/actions` kinds, run by the desktop
  like navigation), in the library and the editor; generated types
  regenerated.

## 9. Licenses and downloads

| Component | License | How it arrives |
|---|---|---|
| Desktop Duplication, D3D11, Media Foundation | Windows APIs | Built into Windows |
| `windows` crate | MIT / Apache-2.0 | Already a dependency |
| Virtual Display Driver | MIT (+ MS-PL from Microsoft's sample) | **Downloaded on demand** (Windows, step 4), Authenticode-checked; listed in "Licences" as downloaded on request |
| xdg-desktop-portal, PipeWire | LGPL-2.1+ / MIT | The desktop's own |
| GStreamer core, base, good (VP8), bad (VA), pipewire plugin | LGPL-2.1+ | Distribution packages (`.deb` dependencies) |
| libvpx | BSD-3-Clause | Distribution package |
| `gstreamer-rs` bindings | MIT / Apache-2.0 | Crate (cargo-deny, cargo-about) |
| VA-API driver (Mesa, Intel media driver) | MIT | The system's |
| MediaCodec | Android platform (AOSP) | The tablet's |

Nothing paid, no account. x264 and openh264 are not used. Every new
crate is checked for MSRV 1.88 and the license allow-lists before it is
added (`gstreamer-rs`' MSRV to check: section 10).

## 10. Decisions and remaining questions

### Decided (Hugo, 2026-11-27)

1. **USB security**: a per-device confirmation alone is not enough (the
   identity in `hello` can be forged). The desktop checks that the peer
   of a USB connection is the adb server (section 4), on the whole USB
   port if it breaks nothing (recommendation: yes, with test mode
   exempt). On top: a confirmation on the PC the first time, remembered
   only for verified connections, and an always visible indicator (tray
   icon + "Arrêter") while the screen is shared. SECURITY.md says what
   remains (adb's own port).
2. **Protocol**: stays on 5, optional `screen` capability in `hello`.
3. **VDD on/off**: check its control pipe first (section 7: open to
   Everyone, `SETDISPLAYCOUNT`); no elevated scheduled task; failing
   the pipe, an administrator prompt at each start rather than a monitor
   plugged in for good. No VDD code before Hugo's checks on Windows.
4. **Leaving the second screen**: no top-right handle (Windows' close
   button). Long press on a volume key, stop from the PC, maybe a
   hold-only tab at the top centre, tried on the tablet.
5. **Construction order**: common plumbing first, tested with the test
   pattern and the Linux source on the development PC; Windows capture
   next. Delivery order unchanged.
6. **Grid layers**: measured before dropping them (section 5); kept if
   the return to the grid stutters.
7. **Kiosk corner**: the bottom-left area of the picture cannot be
   touched in kiosk mode; documented.
8. **Releases**: rc.1 = steps 1 to 3; VDD in a later 0.5 rc.

### Measured on the SM-T580 (2026-11-28)

Test pattern from the PC (GStreamer, 1728×1080), the decoder prototype
(branch `proto/screen`, throwaway) on the real protocol over USB:

| Codec | Frames/s | Bitrate | Decode (tablet) | Send → ack, p50 / p90 |
|---|---|---|---|---|
| H.264 (`vah264enc` → `OMX.Exynos.avc.dec`) | ~49 | 9.8 Mbit/s | 10.7 ms | 22.8 / 24.9 ms |
| VP8 (`vp8enc` → `OMX.Exynos.vp8.dec`) | ~59.6 | 8.9 Mbit/s | 12.2 ms | 19.5 / 21.6 ms |

- H.264 stayed under 60 fps: the source encoded one requested frame at
  a time (a valve opened for one frame, waited for the next picture, then
  for the VA encoder). **Fixed** (2026-11-28): two pictures are kept in
  the encoder (`ENCODER_AHEAD`), let in one by one by a probe holding
  credits (one back each time the stream takes a frame), besides the
  network's window of two unacknowledged frames. Measured on the PC with
  acks 20 ms after each send, as on the SM-T580 (ignored test
  `screen::gst::tests::throughput`): H.264 49.8 → 57.4 frames/s (the
  pattern pipeline alone gives ~57 here), VP8 60 → 60; encoded frames
  wait < 0.1 ms for the network (p90).
- `KEY_LOW_LATENCY` is refused by the Exynos decoder (as in step 1):
  configured without it.
- **Volume keys** (out of kiosk mode): a long press arrives as
  `onKeyLongPress` after ~250 ms; the key's repeats keep coming after
  the second screen closed, so every volume event is consumed until that
  key is released, otherwise the system's volume panel shows. Checked
  with a real key: no panel. To check again in kiosk mode (lock task).

### Measured on the PC: the real screen (2026-11-28)

GNOME 50 (Wayland), Ryzen AI 7 350 (Radeon 860M, Mesa VA-API), a virtual
monitor of 1728×1080 showing a 60 fps animation maximized, acks 20 ms
after each send (ignored test `screen::gst::tests::screen_cost`):

| Encoder | Frames/s (compositor → sent) | Padboard CPU | gnome-shell CPU |
|---|---|---|---|
| H.264 (`vapostproc ! vah264enc`) | 60 → 60 | 8 % of a core | 13 % |
| VP8 (`videoconvert ! vp8enc`, software) | 60 → 60 | 51 % of a core | 14 % |

PipeWire gives BGRA in system memory (no DMA-BUF asked yet);
`vapostproc` converts it on the GPU, VP8 needs `videoconvert` on the CPU
(part of its cost). A still screen costs nothing but the 2 repeats a
second.

### Remaining

1. **GPU and drivers on Windows**: hybrid laptops (Duplication on the
   wrong adapter), hardware MFT quirks (some refuse low latency or CBR);
   the software MFT is the fallback, CPU cost to measure.
2. **Linux encoders**: VA-API H.264 encode missing on some setups; VP8
   fallback costs about half a core at 60 fps (measured above). `VIRTUAL` source on GNOME and KDE 6 only.
3. **Latency over Wi-Fi** and with the TLS + WebSocket path on the
   SM-T580 (OkHttp copies each message; 1.5 MB/s is small, to measure).
4. **Decoder holding one frame**: the repeat-last-frame trick (section 3)
   to confirm on the real tablet.
5. **Volume keys**: that LineageOS 19.1 delivers the long press to the
   app in lock task mode with the screen on (to try on the SM-T580).
6. **Audio**: not in scope (the PC keeps its sound).
7. **gstreamer-rs MSRV** against Rust 1.88, and its size in the `.deb`.
8. **Antivirus**: a program that captures the screen and injects input
   looks like remote-control software; VirusTotal checks of the rc
   builds, as usual.
9. **VDD checks on Windows** (section 7).

## 11. Tests

- Rust: stream framing and flow control (drop before encode, ack window),
  ticket expiry and binding, the USB peer check (parsing of
  `/proc/net/tcp` rows and of the Windows table, from fixtures; the
  decision from peer, listener and uid), protocol fixtures (Rust +
  Kotlin), touch → mouse mapping (pure functions), pin script for VDD.
- Tablet: JVM tests of the stream client against a fake PC, androidTest
  of the second screen (corner free, leaving it with a volume key),
  measurements on the SM-T580 in release (latency acks, `dumpsys
  gfxinfo`, the return to the grid).
- Desktop e2e: the action in the editor; the stream in test mode with the
  test pattern source (no portal, no driver), the confirmation dialog and
  the "Arrêter" indicator.
- Manual: `windows-test-plan.md` (Duplication, MF encoder, the USB peer
  check with another program connecting, VDD install, pipe and removal,
  Bitdefender), `tablet-test-plan.md` (USB and Wi-Fi, volume key exit).
