# Install Padboard on Linux

A `.deb` package for **Ubuntu 22.04+ and Debian 12+** (x86-64), on X11 or
Wayland. You also need an Android tablet with the Padboard app
([tablet guide](install-tablet.md)).

## 1. Install the desktop app

Download `Padboard_<version>_amd64.deb` and `SHA256SUMS` from the
[latest release](https://github.com/Hugoambiehl/Padboard/releases/latest),
then:

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./Padboard_<version>_amd64.deb
```

Start it from the applications menu (*Padboard*). The app speaks English or
French, following the system language.

## 2. Allow keyboard shortcuts (uinput)

Padboard types keyboard shortcuts through a **uinput virtual keyboard**,
which works on X11 and Wayland, in every application. It needs write access
to `/dev/uinput`. Without it, the app runs but shortcut buttons fail with an
error pointing here.

Padboard does **not** ask you to join the `input` group (that would let
programs read every input device, your keyboard included). Instead, a udev
rule tags `/dev/uinput` with `uaccess`: systemd-logind then gives access to
the user of the **active local session** only, and takes it back when the
session ends or another user switches in.

Install the rule once:

```sh
echo 'KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

No reboot or logout is needed. Check it: `getfacl -p /dev/uinput` must list
your user with `rw-`.

- The file name must sort before `73-seat-late.rules`, where `uaccess` tags
  become permissions: hence `70-`.
- `static_node=uinput` makes `/dev/uinput` exist at boot, before the `uinput`
  module is loaded.

> With this rule, any program running in your session can create a virtual
> keyboard and type keys (it cannot read your real keyboard): the same trust
> level as an X11 session.

To remove it:

```sh
sudo rm /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

Keys are stored by **physical position**: a shortcut recorded on an AZERTY
keyboard is the same keys on a QWERTY one.

## 3. Notification area

Padboard lives in the notification area: closing the window keeps it running
for the tablet, and its icon opens the editor again or quits. To start it
with your session: *Connections* (the tablet pill at the top right) →
*Startup* → *Start Padboard when I sign in*.

On **GNOME** the icon needs the *AppIndicator and KStatusNotifierItem
Support* extension: Ubuntu ships it on; on other distributions install it
(`gnome-shell-extension-appindicator` on Debian) and turn it on in
*Extensions*. Without any notification area, closing the window quits
Padboard, as a regular app.

## 4. Connect the tablet over USB

USB needs no pairing and no network.

1. Install **adb**: `sudo apt install adb` (built from the Android Open
   Source Project, Apache-2.0). Padboard finds it in the `PATH`, or choose
   its location in *Connections* → *USB devices*.
2. On the tablet, turn on **USB debugging** (see the
   [tablet guide](install-tablet.md#usb-debugging)), plug it in and accept
   the *Allow USB debugging?* prompt.
3. *Connections* → *USB devices* reads *Ready: port 47321 redirected* and the
   tablet connects by itself.

If the row says *Access to the device denied*, your system lacks the udev
rules for Android devices: `sudo apt install android-sdk-platform-tools-common`.

## 5. Or connect it over Wi-Fi

1. In Padboard, click **Pair a device** → **Turn on Wi-Fi**. If a firewall
   is on (`ufw`), allow TCP 47322 and mDNS (UDP 5353) from your local network.
2. On the tablet, pick your PC in the list (or type its address), then type
   the 6-digit code shown on the PC. **Check that both screens show the same
   fingerprint** (`XXXX-XXXX-XXXX`): if they differ, cancel.
3. The tablet then reconnects by itself, and switches between USB and Wi-Fi
   without a gap.

## Good to know

- **Media buttons** drive the player that is playing (MPRIS: Spotify,
  browsers, VLC…), else send the media key.
- **Volume and microphone** go through PulseAudio or PipeWire
  (`pipewire-pulse`, the default on recent Ubuntu and Debian).
- **Lock** uses `loginctl lock-session`.
- **OBS Studio**: *Tools* → *WebSocket Server Settings* → *Enable WebSocket
  server*, then copy the port and password into *Connections* → *OBS Studio*.
- The configuration and the log of the last launch (`padboard.log`) are in
  `~/.config/io.github.hugoambiehl.padboard/`.
- **Shared PCs**: any program running on the PC can connect to the USB port
  without pairing, like the tablet does through adb. On a PC used by several
  accounts at the same time, another account could send keyboard shortcuts
  into your session while Padboard runs.

## Uninstall

Turn *Startup* off if you enabled it, quit Padboard, then
`sudo apt remove padboard`. Your configuration stays in
`~/.config/io.github.hugoambiehl.padboard/`; delete that folder to remove it.
