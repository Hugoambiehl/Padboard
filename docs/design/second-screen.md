# Second screen (#20): design

Status: **draft for review, no code yet.** Target: 0.5.
Inputs: issue #20 (idea, license review, decision of 2026-11-27) and the
throwaway prototype on branch `proto/mirror` (step 1, Linux → SM-T580 over
USB, measured 2026-11-26).

## 1. What the user gets

A new kind of button, **"Second écran"**. Pressing it on the tablet turns
the tablet into a display of the PC; touches act as the mouse. Pressing
"Boutons" on the tablet (or stopping it from the PC) brings the grid back.
One install only: everything is in Padboard, or downloaded by Padboard on
demand (the Windows virtual display driver), free and open source.

## 2. Order of delivery

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
- The `.deb` adds the distribution's free packages to `depends` /
  `recommends` (as it does for `adb`): `gstreamer1.0-pipewire`,
  `gstreamer1.0-plugins-base`, `-good` (VP8), `-bad` (VA), `gstreamer1.0-vaapi`
  not needed (the `va` plugin is in `-bad`). Checked at run time: a
  missing element gives a clear message, never a crash.
- Wayland and X11 GNOME, KDE Plasma: the portal works on all three; the
  `VIRTUAL` source is GNOME (mutter) and KDE 6 only. Elsewhere: mirror
  only, said so.

### Damage-driven frames

Both systems send a frame only when the screen changes (GNOME: ~10 fps
while scrolling by hand in the prototype). The SM-T580's decoder keeps
one frame inside, so the **last change would stay invisible until the next
one**. The PC therefore repeats the last captured frame (encoded again,
tiny P-frame) one frame time after the last change, once. A static screen
then costs nothing.

## 4. Transport

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
- **Leaving it**: a small "Boutons" handle in the **top-right corner**
  (faded, alpha only; a 1 s hold opens "Retour aux boutons"), and the
  PC's own "Arrêter le second écran" (USB card / tray). Never a system
  gesture (kiosk mode has none).
- **Kiosk corner**: the bottom-left 40 dp stay free as on every screen:
  touches that **start** in the corner are not sent to the PC, so the
  5 s exit gesture works and never holds the mouse down on the PC's
  start button. `KioskCornerTest` covers the new screen.
- Memory: decoder buffers ~6 × 2.8 MB; the grid's cached layers are
  dropped while the stream runs (2 GB device).

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
demand, never bundled.

- **Download**: the official signed release of
  `VirtualDrivers/Virtual-Display-Driver` on GitHub, version and SHA-256
  pinned in the code (bumped by a script, like
  `platform-tools-pin.sh`); each `.dll`, `.cat`, `.sys` checked for a
  valid Authenticode signature by the SignPath Foundation (signer **and**
  issuer).
- **Dialog first**: what it is (a virtual monitor driver, a separate
  project), MIT and MS-PL, not made by Padboard, an administrator prompt
  follows, how to remove it.
- **Install**: Padboard starts itself elevated once (`runas`,
  `padboard-desktop.exe --install-display-driver <dir>`): `pnputil
  /add-driver MttVDD.inf /install` and a root-enumerated device created
  with SetupAPI (`SetupDiCreateDeviceInfo` + `DIF_REGISTERDEVICE` +
  `UpdateDriverForPlugAndPlayDevices`). Removal from Padboard: device
  removed, `pnputil /delete-driver`.
- **Monitor on demand**: the virtual monitor should exist only while the
  second screen runs (Windows moves windows onto it otherwise). How to
  turn it on and off **without an administrator prompt each time** is
  the main open question (VDD's control pipe, its settings file and a
  reload, or disabling the device: needs admin). See section 10.
- Mode list written to VDD's settings (1728×1080 @ 60 first).

## 8. Protocol changes (`crates/protocol`)

- `hello` gains an optional `screen` capability (frozen core: only an
  optional field): codecs, max width/height per codec, whether touch is
  sent. Tablets without it never see a second screen button work (the
  PC answers the press with a clear error on the tablet).
- New messages on the session (`type` names to settle):
  - PC → tablet `screen:offer` { ticket, codec, width, height, fps };
  - tablet → PC `screen:stop` (the "Boutons" handle);
  - PC → tablet `screen:stopped` { reason } (PC stopped, capture lost,
    consent refused on Linux…).
- The stream connection's binary format (section 4), versioned by a
  first byte, documented next to the types, with fixtures tested in Rust
  and Kotlin like the others.
- **Version**: new messages only reach tablets that announced `screen`,
  so nothing breaks for a 0.4 tablet. Two options: keep protocol 5
  (capability-gated, a 0.4 tablet keeps working with a 0.5 PC) or bump
  to 6 (simpler reasoning, but a 0.4 tablet gets an update-only session
  until it is updated). **Proposed: stay on 5**, decided before coding.
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

## 10. Risks and open questions

1. **Screen content over USB loopback.** USB sessions are trusted without
   PIN, and any local process can reach `127.0.0.1:47321` (accepted risk,
   SECURITY.md: another account on the PC could send keys). With a second
   screen it could **read the screen**. Options: a confirmation on the PC
   the first time a device starts a stream (remembered per paired
   device), or second screen over USB only for devices also paired over
   Wi-Fi, or accept and document. To decide.
2. **VDD monitor on/off without admin** (section 7). If impossible, the
   monitor stays plugged once installed (windows may land on it while
   the tablet shows the grid) or each start asks for admin. To check on
   Windows 10 and 11, with Secure Boot, and Bitdefender's reaction to the
   driver install.
3. **GPU and drivers on Windows**: hybrid laptops (Duplication on the
   wrong adapter), hardware MFT quirks (some refuse low latency or CBR);
   the software MFT is the fallback, CPU cost to measure.
4. **Linux encoders**: VA-API H.264 encode missing on some setups; VP8
   fallback costs ~1.2 cores. `VIRTUAL` source on GNOME and KDE 6 only.
5. **Latency over Wi-Fi** and with the TLS + WebSocket path on the
   SM-T580 (OkHttp copies each message; 1.5 MB/s is small, to measure).
6. **Decoder holding one frame**: the repeat-last-frame trick (section 3)
   to confirm on the real tablet.
7. **Protocol version** (section 8): stay on 5 or bump to 6.
8. **Exit handle**: top-right "Boutons" handle vs. another gesture; to
   try by hand on the tablet.
9. **Audio**: not in scope (the PC keeps its sound).
10. **gstreamer-rs MSRV** against Rust 1.88, and its size in the `.deb`.
11. **Antivirus**: a program that captures the screen and injects input
    looks like remote-control software; VirusTotal checks of the rc
    builds, as usual.

## 11. Tests

- Rust: stream framing and flow control (drop before encode, ack window),
  ticket expiry and binding, protocol fixtures (Rust + Kotlin), touch →
  mouse mapping (pure functions), pin script for VDD.
- Tablet: JVM tests of the stream client against a fake PC, androidTest
  of the second screen (corner free, leaving it), measurements on the
  SM-T580 in release (latency acks, `dumpsys gfxinfo`).
- Desktop e2e: the action in the editor; the stream in test mode with a
  test pattern source instead of the screen (no portal, no driver).
- Manual: `windows-test-plan.md` (Duplication, MF encoder, VDD install
  and removal, Bitdefender), `tablet-test-plan.md` (USB and Wi-Fi).
