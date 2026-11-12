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
Consequence: any program running on the PC, under any user account, can
connect to this port and send actions. On a PC used by several people at the
same time, another account could send keyboard shortcuts into your session
while Padboard runs.

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

**Kiosk mode.** On the tablet, Padboard becomes the device owner. It can
leave this role at any time: from the PC, or by holding the top-right
corner for 5 seconds. It requests no other permission than the network.

**Not in scope.** Padboard does not protect against malware already running
on the PC, or someone with physical access to an unlocked PC or tablet.
