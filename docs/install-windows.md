# Install Padboard on Windows

Windows 10 or 11, 64-bit. You also need an Android tablet with the Padboard
app ([tablet guide](install-tablet.md)).

## 1. Install the desktop app

1. Download `Padboard_<version>_x64-setup.exe` from the
   [latest release](https://github.com/Hugoambiehl/Padboard/releases/latest).
2. Optional: check the download. In PowerShell,
   `Get-FileHash .\Padboard_<version>_x64-setup.exe` must print the SHA-256
   listed for that file in the release's `SHA256SUMS`.
3. Run the installer. It is **not signed yet**, so SmartScreen shows
   *Windows protected your PC*: click *More info* → *Run anyway*.
   It installs for your user only (no administrator rights) and adds a Start
   menu entry. WebView2 comes with Windows 11; on Windows 10 the installer
   downloads it if it is missing.

The installer speaks English or French, following the language of Windows;
so does the app.

Padboard lives in the **notification area** (the icons next to the clock,
maybe under the `^` arrow): closing the window keeps it running for the
tablet. Use the icon to open the editor again or to quit. To start it with
Windows: *Connections* (the tablet pill at the top right) → *Startup* →
*Start Padboard when I sign in*.

## 2. Connect the tablet over USB

USB needs no pairing and no network: the cable is enough.

1. **adb** carries the connection over the cable. Install Google's
   Platform-Tools: `winget install Google.PlatformTools` (or download them
   from developer.android.com), then sign out and back in so Padboard finds
   `adb` in the `PATH`. Or point Padboard to `adb.exe`: *Connections* →
   *USB devices* → *Choose the adb location…*.
   Note: the Platform-Tools binaries come under Google's Android SDK license,
   not a free software license.
2. On the tablet, turn on **USB debugging** (see the
   [tablet guide](install-tablet.md#usb-debugging)).
3. Plug the tablet in. If `adb devices` lists nothing, install your tablet
   maker's USB driver (for Samsung: *Android USB Driver for Windows*).
4. Accept the *Allow USB debugging?* prompt on the tablet.
5. *Connections* → *USB devices* reads *Ready: port 47321 redirected* and the
   tablet connects by itself.

## 3. Or connect it over Wi-Fi

1. The Windows network must be **Private**: *Settings* → *Network &
   Internet* → *Wi-Fi* (or *Ethernet*) → your network → *Private network*.
   On a public network, Windows blocks the discovery of the PC.
2. In Padboard, click **Pair a device** → **Turn on Wi-Fi**. The first time,
   allow Padboard through the Windows firewall (*Private networks*).
3. On the tablet, pick your PC in the list (or type its address), then type
   the 6-digit code shown on the PC. **Check that both screens show the same
   fingerprint** (`XXXX-XXXX-XXXX`): if they differ, another device is
   pretending to be your PC, so cancel.
4. The tablet then reconnects by itself, and switches between USB and Wi-Fi
   without a gap when you plug or unplug the cable.

Wi-Fi uses TCP port 47322 (TLS) and mDNS (UDP 5353). Paired tablets are
listed in *Connections* → *Paired devices*, where you can revoke them.

## 4. Set up OBS Studio (optional)

In OBS 28 or later: *Tools* → *WebSocket Server Settings* → *Enable
WebSocket server*, then copy the port and the password into Padboard:
*Connections* → *OBS Studio* → *Control OBS*.

## Good to know

- Keyboard shortcuts do not reach windows run as administrator (a Windows
  rule, UIPI).
- The log of the last launch is `padboard.log` in
  `%APPDATA%\io.github.hugoambiehl.padboard\`, with the configuration.
- **Shared PCs**: any program running on the PC can connect to the USB
  port without pairing, like the tablet does through adb. On a PC used by
  several accounts at the same time, another account could send keyboard
  shortcuts into your session while Padboard runs.

## Uninstall

Quit Padboard (notification area icon → *Quit Padboard*), then *Settings* →
*Apps* → *Installed apps* → *Padboard* → *Uninstall*. Tick *Delete the
application data* to remove your configuration too
(`%APPDATA%\io.github.hugoambiehl.padboard\`); otherwise it stays for a
later install. Turn *Startup* off first if you enabled it.
