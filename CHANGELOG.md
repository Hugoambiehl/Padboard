# Changelog

Notable changes of Padboard, generated from the commit history with
[git-cliff](https://git-cliff.org). Versions follow [Semantic Versioning](https://semver.org).

## 0.2.0-rc.4 (2026-11-16)

### Upgrading from 0.1.0

0.1.0 cannot update itself: install this version by hand once. On Windows, run the new installer over the old version (your configuration is kept); on Linux, `sudo apt install ./Padboard_<version>_amd64.deb`. Then plug the tablet in over USB and click **Update** next to it in the PC's connections view (USB devices card). From then on, the PC looks for new versions by itself and updates the tablet.

### Features

- **desktop**: Look for new versions and install them on Windows
- **protocol**: Protocol 5 with a frozen core and update-only sessions
- **desktop**: Update USB tablets to the PC's version
- Update kiosk tablets over Wi-Fi from the PC
- **desktop**: Explain why a published tablet cannot use a development PC
- **android**: Say when the paired PC is visible but unreachable
- **desktop**: Say when a tablet's Wi-Fi session keeps dropping
- **desktop**: Open the update wizard and say when an update did not install
- **desktop**: Offer to download Google's adb on Windows
- **desktop**: Detect another adb's server and a server that keeps stopping
- **kiosk**: Report who took the tablet out of kiosk mode

### Fixes

- **desktop**: Keep the session autostart entry per user
- Never print a secret through Debug or toString
- **desktop**: Keep the signed test fixtures byte for byte on Windows
- **desktop**: Offer Wi-Fi updates as soon as a tablet enters kiosk mode
- **desktop**: Neutral message when the update and the tablet app keys differ
- **android**: Show an update above the screensaver
- **android**: Tell the user when an update is cut off midway
- **android**: Pace the pieces of an update over Wi-Fi
- **desktop**: Keep a failed update install shown after an automatic check
- **linux**: Make the .deb depend on ca-certificates
- **desktop**: Say that system test mode uses USB
- **kiosk**: Move the exit gesture to an empty corner, clearer confirmation
- **desktop**: Do not wait 2 s per probe of a missing adb server on Windows
- **desktop**: Remove adb stops only a server started from Padboard's copy
- **desktop**: A double-click no longer releases a tablet from kiosk mode
- **scripts**: Tablet UI tests see a running Padboard again
- **updates**: Never start an unsigned Windows installer from Padboard
- **windows**: Keep the configuration and autostart across updates

## 0.1.0 (2026-11-12)

Initial release.

- **Desktop app** (Windows 10/11, Linux): an editor for the tablet's buttons, with pages, folders, profiles, drag and drop, undo and redo, Lucide icons and colors. It lives in the notification area and can start with the session.
- **Actions**: keyboard shortcuts, media controls, volume and sound output mute, microphone mute, opening an app, a file or a website, locking the session, multi actions, page and folder navigation, and OBS Studio (scenes, recording, streaming, audio inputs). Buttons show live states: microphone muted, active scene, REC, LIVE.
- **Tablet app** (Android 8+): the button grid with page swipes and folders, over USB (no setup) or Wi-Fi (TLS, PIN pairing with a key fingerprint check), automatic reconnection and USB/Wi-Fi switch, kiosk mode for a dedicated tablet, screensaver with a clock.
- English and French.

