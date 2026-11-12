# Set up the tablet

Any Android tablet running **Android 8.0 or later** works; no Google
services are needed. A dedicated tablet works best: Padboard can lock it in
**kiosk mode** so it only ever shows your buttons. Install the desktop app
first ([Windows](install-windows.md), [Linux](install-linux.md)).

## Install the app

Download `padboard-<version>.apk` from the
[latest release](https://github.com/Hugoambiehl/Padboard/releases/latest)
(check it against `SHA256SUMS`), then either:

- **From the PC** (USB debugging on, see below):
  `adb install padboard-<version>.apk`;
- **On the tablet**: open the APK from the browser or a file manager and
  allow that app to *install unknown apps* when Android asks.

Updates install the same way, over the existing app (`adb install -r …`):
your pairing is kept. Every release is signed with the same key; its
certificate's SHA-256 is
`63:7B:B3:C0:7A:1B:C2:A0:C1:0D:92:F4:15:2E:F9:C9:2F:BF:00:C9:D5:10:B3:E7:2F:63:F7:C2:E3:DC:FA:46`
(`apksigner verify --print-certs padboard-<version>.apk`).

## USB debugging

USB is the simplest connection: no pairing, no network, and the tablet
charges. It needs USB debugging:

1. *Settings* → *About tablet* → *Software information* → tap *Build number*
   seven times ("You are now a developer").
2. *Settings* → *Developer options* → turn on *USB debugging*.
3. Plug the tablet into the PC and accept *Allow USB debugging?* (tick
   *Always allow from this computer*).

The menu names vary a little between brands. Padboard on the PC then
redirects the connection over the cable by itself and the tablet connects:
the pill at the top right reads *USB*.

## Wi-Fi

Without the cable, the tablet connects over Wi-Fi (TLS encryption, PC and
tablet on the same network). Pair it once:

1. On the PC: **Pair a device** → **Turn on Wi-Fi**: a 6-digit code shows up.
2. On the tablet: pick the PC in the list (or *Type the PC's address*), then
   type the code.
3. **Check that both screens show the same fingerprint** (`XXXX-XXXX-XXXX`).
   If they differ, cancel: another device is pretending to be your PC.

The tablet then finds the PC by itself, reconnects after a cut, and switches
to USB when you plug the cable in (and back to Wi-Fi when you unplug it).
A tablet is paired with one PC; tap the pill at the top right for *Pair
another PC*.

## Using it

- **Swipe** to change page; folders open over the grid and close with the
  back button.
- Buttons react when you touch them, and run when you lift your finger
  without swiping (or when you hold them still).
- Buttons show the PC's state: microphone muted, sound muted, playing, OBS
  scene, REC, LIVE.
- The **screensaver** (set on the PC: *Connections* → *Tablet screensaver*)
  dims the tablet to a clock after a while; the first touch only wakes it.
- If the PC goes away, the grid stays, dimmed, while the tablet looks for it.
- *About and licenses*: from the setup screen, or the menu of the pill.

## Kiosk mode

For a tablet dedicated to Padboard: the app becomes the tablet's **device
owner**. It stays on screen (no home, recent apps, notifications or status
bar; the power menu still works), is the home screen (it starts at boot),
turns the lock screen off and keeps the screen on (the screensaver dims it).

Requirements: **no account** on the tablet (*Settings* → *Accounts*: remove
the Google or Samsung accounts first) and a single user.

### Turn it on

Tablet plugged in over USB: Padboard on the PC → *Connections* → *USB
devices* → **Turn on kiosk mode**. The row then reads *Kiosk mode on*. If
another app already manages the tablet, or it still has an account, the row
says so.

By hand, the same thing:

```sh
adb shell dpm set-device-owner io.github.hugoambiehl.padboard/.kiosk.KioskAdmin
adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity
```

On LineageOS 19+ (Android 12L), the taskbar stays drawn over apps locked on
screen; Padboard's button turns it off. By hand:
`adb shell content insert --uri content://lineagesettings/system --bind name:s:enable_taskbar --bind value:s:0`
(`value:s:1` brings it back).

### Turn it off

Either of these undoes every setting and gives up the device owner role:
the tablet becomes an ordinary tablet again, and Padboard can be
uninstalled.

- From the PC: *Connections* → *Connected tablets* → **Release the tablet**
  (click twice).
- On the tablet, even without a PC: hold the **top-right corner** (the clock)
  for 5 seconds, then *Leave kiosk mode*.

Updates (`adb install -r`) work in kiosk mode: the app comes back on screen
right after.

### If the app no longer starts

While it is the device owner, Padboard cannot be uninstalled. In this order:

1. Install a fixed version over it (`adb install -r …`): updates are allowed
   in kiosk mode.
2. If the app starts at all, use the corner gesture (or start it with
   `adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity`,
   then the gesture).
3. Last resort: a **factory reset** from the recovery menu (this wipes the
   tablet). The key combination depends on the model (Samsung with a home
   button: hold *Volume up + Home + Power* from power off), then *Wipe data /
   factory reset*.

## Uninstall

Leave kiosk mode first if it is on, then uninstall Padboard like any app
(or `adb uninstall io.github.hugoambiehl.padboard`).
