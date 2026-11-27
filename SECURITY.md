# Security

Padboard lets a tablet send keystrokes and commands to a PC, so its
security matters. Thank you for helping keep it safe.

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through GitHub:
[Security → Report a vulnerability](https://github.com/Hugoambiehl/Padboard/security/advisories/new).
Describe the problem, the affected version and platform, and how to
reproduce it.

Padboard is maintained by volunteers: expect a first answer within a week.
Once a fix is released, the advisory is published, with credit to you if
you wish.

## Supported versions

Only the latest release gets security fixes. Update both the desktop app and
the tablet app.

## Security model

What Padboard protects, and what it trusts.

**USB connection.** The desktop app listens on `127.0.0.1:47321` only, and
forwards that port to the tablet with `adb reverse`. Connections on this
port are accepted **without pairing**: reaching it requires running code on
the PC, or a device plugged in with USB debugging authorized on the PC.
Up to 0.4, any program running on the PC, under any user account, could
connect to this port and send actions: on a PC used by several people at the
same time, another account could send keyboard shortcuts into your session
while Padboard runs.

**Only the adb server may connect to the USB port (since 0.5).** A real USB
connection always comes from the **adb server** (`adb reverse`). The desktop
checks the process on the other end of each connection to the USB port
(Linux: `/proc/net/tcp`, then the owning process, or the owning account when
the adb server runs under another one, e.g. started with `sudo`; Windows: the
TCP table) and refuses it unless it is the process listening on the adb
server's port. A refused connection is logged with the program and shown in
the USB card; one that cannot be checked is refused too, and both the PC and
the tablet say what to do. This closes the direct path described above.

What remains: the adb server itself accepts commands from any local account
(adb's own design), so on a PC shared between several accounts another
account can still drive the tablet through adb, and through it reach
Padboard. Do not use USB debugging on a PC whose other accounts you do not
trust.

**Second screen (Padboard 0.5, in development).** It sends the PC's picture
to the tablet
([design](docs/design/second-screen.md#usb-check-the-process-on-the-other-end)):

- The first time a device starts a second screen, the PC asks for
  confirmation. The answer is remembered only for a verified connection
  (paired Wi-Fi, or USB that passed the check above).
- While the screen is shared, the tray icon shows it, with "Arrêter le
  second écran".
- On Windows, the extended screen uses
  [Virtual Display Driver](https://github.com/VirtualDrivers/Virtual-Display-Driver),
  downloaded on request. Its control channel (a named pipe) is open to every
  local account: any of them can change its settings while it is installed.
  Padboard can remove it.

**Wi-Fi connection** (off until the user turns it on).

- TLS on port 47322, with a self-signed certificate. The tablet pins the
  SHA-256 of the PC's public key at pairing and refuses any other key
  afterwards.
- Pairing needs a 6-digit code shown on the PC on demand. The code is
  single-use, expires after 2 minutes, and allows 5 attempts. Both screens
  show a 60-bit fingerprint of the PC's key to compare, against a
  machine-in-the-middle during pairing.
- The tablet then gets a 32-byte random token. The PC stores only its
  SHA-256 and compares it in constant time; tokens are revocable from the PC.
- Before authentication a client may only ask to pair. Unauthenticated
  connections are capped and time out, so pairing attempts cannot lock a
  paired tablet out.

**Both ports** refuse WebSocket handshakes that carry an `Origin` header, so
a web page open on the PC cannot drive Padboard.

**On the PC.** The configuration files that hold secrets (TLS key, tokens,
OBS password) are readable by their owner only. On Linux, keyboard
shortcuts go through `/dev/uinput`. The documented udev rule gives access to
the user of the active local session only, but then any program in that
session can create a virtual keyboard, as on X11
([details](docs/install-linux.md#2-allow-keyboard-shortcuts-uinput)).

**Releases.** The tablet APK, the desktop updates and the checksums
(`SHA256SUMS`) are signed on the maintainer's PC, with keys that never go
to CI; the apps refuse anything else. The Windows installer has **no
Authenticode signature** (no code signing certificate for now): SmartScreen
warns on first launch, and Padboard never starts an installer by itself. It
downloads it, checks its signature and checksum, then shows it for the user
to run. Check a manual download with the release's `SHA256SUMS`.

**Kiosk mode.** On the tablet, Padboard becomes the device owner. It can
leave this role at any time: from the PC, or by holding the bottom-left
corner for 5 seconds, then confirming. It requests no other permission than the network.

**Not in scope.** Padboard does not protect against malware already running
on the PC, or someone with physical access to an unlocked PC or tablet.
