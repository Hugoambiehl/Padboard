# Changelog

Notable changes of Padboard, generated from the commit history with
[git-cliff](https://git-cliff.org). Versions follow [Semantic Versioning](https://semver.org).

## 0.2.2 (2026-11-26)

### Fixes

- **desktop**: Explain refused Platform-Tools downloads
- **desktop**: Pin the republished Platform-Tools 37.0.3 archive
- **deps**: Update cookie and url for RUSTSEC-2026-0594 and RUSTSEC-2026-0592
- **release**: Find the APK signing properties in their default place

## 0.2.1 (2026-11-19)

### Features

- **release**: Show SecureAge under its VirusTotal site name in verify-release
- **desktop**: Keep the previous launch's log as padboard.log.1

### Fixes

- **obs**: Refresh the editor's scene and input lists when OBS sends them
- **installer**: Skip the directory page when Padboard is installed
- **installer**: Correct the French texts of the installer
- **release**: Version release candidates' .deb with a tilde
- **obs**: Ask OBS nothing until it has finished starting

## 0.2.0 (2026-11-17)

### Upgrading from 0.1.0

0.1.0 cannot update itself: install this version by hand once. On Windows, run the new installer over the old version (your configuration is kept); on Linux, `sudo apt install ./Padboard_<version>_amd64.deb`. Then plug the tablet in over USB and click **Update** next to it in the PC's connections view (USB devices card). From then on, the PC looks for new versions by itself and updates the tablet.

### Features

- **desktop**: Look for new versions; on Windows, download the installer, check it and show it
- **protocol**: Protocol 5 with a frozen core and update-only sessions
- **desktop**: Update USB tablets to the PC's version
- Update kiosk tablets over Wi-Fi from the PC
- **android**: Say when the paired PC is visible but unreachable
- **desktop**: Say when a tablet's Wi-Fi session keeps dropping
- **desktop**: Say when an update did not install
- **desktop**: Offer to download Google's adb on Windows
- **desktop**: Detect another adb's server and a server that keeps stopping
- **kiosk**: Report who took the tablet out of kiosk mode

### Fixes

- **desktop**: Keep the session autostart entry per user
- Never print a secret through Debug or toString
- **linux**: Make the .deb depend on ca-certificates
- **kiosk**: The exit gesture moves to the bottom-left corner, with a clearer confirmation
- **desktop**: A double-click no longer releases a tablet from kiosk mode
- **windows**: Keep the configuration and autostart across updates

## 0.1.0 (2026-11-12)

Initial release.

- **Desktop app** (Windows 10/11, Linux): an editor for the tablet's buttons, with pages, folders, profiles, drag and drop, undo and redo, Lucide icons and colors. It lives in the notification area and can start with the session.
- **Actions**: keyboard shortcuts, media controls, volume and sound output mute, microphone mute, opening an app, a file or a website, locking the session, multi actions, page and folder navigation, and OBS Studio (scenes, recording, streaming, audio inputs). Buttons show live states: microphone muted, active scene, REC, LIVE.
- **Tablet app** (Android 8+): the button grid with page swipes and folders, over USB (no setup) or Wi-Fi (TLS, PIN pairing with a key fingerprint check), automatic reconnection and USB/Wi-Fi switch, kiosk mode for a dedicated tablet, screensaver with a clock.
- English and French.

