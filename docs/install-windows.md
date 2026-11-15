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

USB is optional: over Wi-Fi (section 3) you need nothing else.

1. **adb** carries the connection over the cable. It is part of Google's
   *Platform-Tools*, which are **not free software** (Android SDK License
   Agreement: no redistribution, use granted "to develop applications"),
   so Padboard does not include them. Two ways to get it:
   - **From Padboard**: *Connections* → *USB devices* → **Install adb…**.
     The dialog sums up Google's license and links to it; once you accept
     it, Padboard downloads a fixed version from Google (about 8 MB),
     checks its SHA-256 and the *Google LLC* signature of every program,
     and puts it in `%LOCALAPPDATA%\io.github.hugoambiehl.padboard\platform-tools\`.
     *Remove adb* deletes it. If Google no longer offers that version,
     Padboard says so: use the other way.
   - **By yourself**: `winget install Google.PlatformTools` (or the zip from
     [Google's page](https://developer.android.com/tools/releases/platform-tools)),
     then sign out and back in so Padboard finds `adb` in the `PATH`, or
     point Padboard to `adb.exe` (*Choose the adb location…*). An `adb`
     in the `PATH` is used before the one Padboard downloaded.
2. On the tablet, turn on **USB debugging** (see the
   [tablet guide](install-tablet.md#usb-debugging)).
3. Plug the tablet in. Windows 10 and 11 usually recognize it by themselves
   (no driver to install); otherwise see
   [If adb does not see the tablet](#if-adb-does-not-see-the-tablet).
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

## If adb does not see the tablet

Plugged in with USB debugging on, the tablet should appear in *USB
devices* (and in `adb devices`). Windows normally installs a generic
driver by itself (checked with a Galaxy Tab A 10.1 under LineageOS 19.1
on Windows 11). If nothing shows up:

1. Try another cable (some only charge) and another USB port, and unlock
   the tablet: the *Allow USB debugging?* prompt only shows on an unlocked
   screen.
2. On the tablet, *Settings* → *System* → *Developer options*: *Revoke USB
   debugging authorizations*, then plug it in again and accept the prompt.
3. **As a last resort**, install the maker's driver. For Samsung: the
   *Samsung Android USB Driver for Windows*
   ([developer.samsung.com](https://developer.samsung.com/android-usb-driver)),
   then unplug and plug the tablet again. Other makers: see Google's
   [list of OEM USB drivers](https://developer.android.com/studio/run/oem-usb).
   In the Device Manager, the tablet should then appear as an *ADB
   Interface* rather than an unknown device.

## Updates

Padboard looks for a new version at launch and once a day (*Connections* →
*About* → *Updates*; only the list of releases is asked from GitHub). When
one is available, **Install** downloads it, checks its signature, then opens
Padboard's installer: follow its steps as for the first install (Padboard
closes meanwhile and your edits are saved first). While the installer is not
signed, it opens as a regular wizard rather than installing silently.

If the install does not finish (wizard closed, or blocked by an antivirus),
Padboard says so at its next launch: try again with *Check now*, or install
the version by hand from its release page.

## If your antivirus blocks Padboard

The installer is not signed yet (a code signing certificate is on its way),
so an antivirus may take it for a threat, mostly when it is started by an
update: Bitdefender's *Advanced Threat Control* (`Atc4.Detection`) has
quarantined a test installer that way. The files of a release are the ones
listed in its `SHA256SUMS`, signed by the maintainer.

1. **Check the file**: in PowerShell, `Get-FileHash` on the installer must
   print the SHA-256 listed in the release's `SHA256SUMS`. If it differs,
   delete the file and do not run it.
2. **Report the false positive** to your antivirus' maker, with the
   installer and the name of the detection. Bitdefender:
   [bitdefender.com/submit](https://www.bitdefender.com/submit), category
   *False Positive*; confirmed false alarms are usually fixed within hours.
   You can also [open an issue](https://github.com/Hugoambiehl/Padboard/issues)
   so the maintainer reports it too.
3. **Meanwhile**, install by hand: download the installer from the release
   page and run it (section 1). If it was quarantined, restore it from the
   antivirus' quarantine only if its SHA-256 is right, and add an exception
   for it.

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
