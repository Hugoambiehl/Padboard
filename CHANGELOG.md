# Changelog

Notable changes of Padboard, generated from the commit history with
[git-cliff](https://git-cliff.org). Versions follow [Semantic Versioning](https://semver.org).

## 0.1.0-rc.1 (2026-11-12)

### Features

- **protocol**: Add v1 message envelope, layout model and fixtures
- **actions**: Add ActionExecutor with hotkey support on Linux and Windows
- **desktop**: Add Tauri app with WebSocket server and status screen
- **android**: Connect by manual address and display the button grid
- **protocol**: Add device identity, authentication and pairing messages
- Speak protocol 2 on the desktop and the tablet
- **desktop**: Persist the PC identity, preferences and paired devices
- **desktop**: Authenticate Wi-Fi clients with PIN pairing and tokens
- **desktop**: Serve Wi-Fi over TLS on its own port and announce it over mDNS
- **desktop**: Forward the USB port to plugged tablets automatically
- **desktop**: Pair, revoke and follow USB devices from the status screen
- **android**: Compute the PC key fingerprint like the desktop
- **android**: Pair over Wi-Fi and switch between USB and Wi-Fi automatically
- **desktop**: Close sessions that stay silent
- **protocol**: Send button icons as stroke path data
- **desktop**: Store profiles, pages and buttons in a validated configuration
- **desktop**: Edit the configuration with undo, live sync and debounced saves
- **desktop**: Editor shell with profile menu, undo and a connections view
- **desktop**: Replica grid with pages, grid size and keyboard editing
- **desktop**: Action library with drag and drop
- **desktop**: Button inspector with icon picker and shortcut recorder
- **desktop**: Separate the edited profile from the displayed one
- **android**: Button icons and swipeable pages on the tablet
- **protocol**: Protocol 3 with folders, button state and navigation
- **actions**: Action kinds of phase 4 and folders in the configuration
- **actions**: Media, volume, microphone, open and lock on Linux
- **actions**: Media, volume, microphone, open and lock on Windows
- **desktop**: Button states pushed to the tablets
- Navigation buttons and folders from end to end
- Multi actions, and navigation that never gets stuck on the tablet
- OBS Studio buttons through obs-websocket
- **desktop**: An inspector form for every kind of action
- **desktop**: The subtitle follows the action while it is the automatic text
- **desktop**: Log to padboard.log in the config directory
- **desktop**: Translated titles for the open action's file pickers
- **android**: Explain a lost connection over the dimmed grid
- Protocol 4 and kiosk mode for the tablet
- Tablet screensaver set from the PC
- **android**: Smoother tile states, folders and presses
- **desktop**: About card with the license and third-party notices
- **android**: About and licenses screen
- Padboard app icon generated from a single SVG
- **desktop**: Texts sent to the tablets follow the PC's language
- **desktop**: English user interface
- **android**: English user interface, French kept in values-fr

### Fixes

- **actions**: Avoid a duplicate field in action logs
- **desktop**: Restore the VS Code snap environment from its saved copies
- **android**: Ellipsize long PC names in the connection pill
- **desktop**: Build the paired devices tests on Windows
- **desktop**: Announce over mDNS on physical interfaces only
- **android**: Keep secrets out of backups and cleartext on loopback only
- **desktop**: Announce over mDNS in IPv4 only
- **desktop**: Build the demo profile on Windows
- **desktop**: Keyboard focus, menus and native controls in WebKitGTK
- **android**: Keep the shown page across reconnections
- **actions**: State example prints only real changes
- **desktop**: Show the tablet's kiosk state and explain refusals
- **android**: Come back in front after an update in kiosk mode
- **deps**: Update time and quick-xml for RustSec advisories
- **android**: About screen rows, long texts and access while searching

### Performance

- **android**: Cached layers for pages and folders
- **android**: Stop drawing while waiting for the PC

