# Windows test plan

Manual checks of a Windows build before a release: install it, connect the
tablet over USB and Wi-Fi, and try every action. The user guide is
[install-windows.md](install-windows.md).

## 1. Get the app

### A. Installer of a release (or release candidate)

Download `Padboard_<version>_x64-setup.exe` from the GitHub release (a draft
release is visible to the maintainers only), or
`gh release download v0.1.0-rc.1 -p '*-setup.exe'`, then run it as in step 3
below. Check it against `SHA256SUMS` (`Get-FileHash <file>` in PowerShell).

### B. Installer built by the CI (no tools on the PC)

1. GitHub → *Actions* → *CI* → *Run workflow* → tick **Also build the Windows
   installer** → *Run workflow* (or `gh workflow run CI -f installer=true` with
   a token allowed to run workflows).
2. When the run is green, download the **padboard-windows-installer**
   artifact (a zip holding `Padboard_<version>_x64-setup.exe`; kept 14 days).
3. Run the installer. It is unsigned: SmartScreen shows *Windows protected
   your PC* → *More info* → *Run anyway*. It installs for the current user
   (no admin rights) and adds a Start menu entry. WebView2 is part of
   Windows 11; on Windows 10 the installer fetches it if missing.

### C. Build on the PC

Tools: Git, Node.js 24, Rust (`rustup`, default MSVC toolchain), and Visual
Studio Build Tools 2022 with *Desktop development with C++* (MSVC and the
Windows SDK; free to use, not open source).

```powershell
git clone <repo> padboard
cd padboard\apps\desktop
npm ci
npx tauri build --bundles nsis    # installer: ..\..\target\release\bundle\nsis\
npm run tauri dev                 # or run it in development mode
```

Configuration and the log of the last launch (`padboard.log`) live in
`%APPDATA%\io.github.hugoambiehl.padboard\`. Padboard keeps the log open
for writing while it runs, so `[IO.File]::ReadAllBytes` (which does not
share writing) fails on it then; read it with shared access:

```powershell
$log = "$env:APPDATA\io.github.hugoambiehl.padboard\padboard.log"
$s = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
$r = New-Object IO.StreamReader($s); $r.ReadToEnd(); $r.Close()
```

## 2. Tablet over USB

1. Install adb (Google Platform-Tools, under the Android SDK license, not a
   free one; see phase 6): `winget install Google.PlatformTools`, then sign
   out and back in so Padboard sees it in `PATH`, or point Padboard to
   `adb.exe` (*Connexions* → *Appareils USB* → *Choisir l'emplacement d'adb…*).
2. Plug the tablet. Windows Update normally installs the Samsung ADB
   interface driver; if `adb devices` lists nothing, install Samsung's *Android
   USB Driver for Windows*.
3. Accept the *Allow USB debugging* prompt on the tablet (new PC = new key).
4. *Connexions* → *Appareils USB* should say *Prête : port 47321 redirigé*,
   and the tablet connects by itself (pill: *USB*).

## 3. Tablet over Wi-Fi

1. The Windows network must be **Private** (*Settings → Network → Wi-Fi →
   your network → Private*), or mDNS discovery is blocked.
2. In Padboard, turn on Wi-Fi (*Connexions* toggle, or *Associer un appareil*
   → *Activer le Wi-Fi*). On the firewall prompt, allow Padboard on **private
   networks** (TCP 47322 and mDNS, UDP 5353).
3. The tablet pairs with one PC: on the tablet, open the connection pill →
   *Oublier ce PC* (it then has to pair again with the Linux PC later).
4. Unplug the cable (or remove the reverse: `adb reverse --remove-all`). The
   tablet lists the Windows PC: choose it, check that the fingerprint shown
   on both sides is the same, type the PIN shown by *Associer un appareil*.
5. The pill says *Wi-Fi*. Plug the cable back: it switches to USB without a
   break.

## 4. Test every action

Use a profile made for the test (or the demo one). For safe experiments,
`$env:PADBOARD_TEST_DIR="C:\Temp\padboard-test"` before starting Padboard from
PowerShell gives a separate configuration whose actions are only recorded
(add `$env:PADBOARD_TEST_SYSTEM="1"` to run them for real); its log is
`app.log` in that folder.

| Action | Do | Expect |
|---|---|---|
| Shortcut | The demo *Explorateur* (Win + E); record Ctrl + Shift + Esc in the inspector | Explorer opens; Task Manager opens. Keys do not reach windows run as administrator (Windows UIPI) |
| Media | Play something in Spotify or YouTube in Edge/Firefox; *Lecture / Pause*, *Suivant*, *Précédent*, *Stop* | The player obeys; the tile shows the pause icon while playing (within a second); with no player, the media key is sent |
| Volume | *Volume +* / *−* (5 %), *Régler à 30 %*, *Muet* | Same value as the Windows volume slider; muted: red *Coupé* tile; change it from Windows: the tablet follows within half a second |
| Microphone | *Micro* on the default **recording** device | Mutes / unmutes (*Settings → Sound → Input*); red tile while muted, also when muted from Windows; no microphone: dimmed tile |
| Open | `https://example.org`, the library's *Documents* button (`shell:Personal`, works with redirected Documents), `%USERPROFILE%\Desktop`, a `.txt` file, `notepad.exe`, a Start menu `.lnk` | Each opens with its default handler |
| Lock | *Verrouiller* | The session locks (as Win + L) |
| Navigation | Next / previous page, folder, back | As on Linux |
| Multi action | Micro, pause 500 ms, volume +, next page | Steps in order; pressing again while it runs does nothing |
| OBS | OBS Studio 28+: *Tools → WebSocket Server Settings* → enable, copy port and password into *Connexions → OBS* | Status *Connecté*; scene buttons (active scene highlighted), *Enregistrer* (REC badge), *Live* with a real service (EN DIRECT badge), source mute (red); OBS closed: OBS tiles dimmed, back by themselves when it reopens |

Also check: the tablet shows the right states right after connecting, after
a cable unplug/replug, and after the PC wakes from sleep.

## 5. Notification area and startup

| Check | Do | Expect |
|---|---|---|
| Tray icon | Start Padboard | The Padboard icon is in the notification area (maybe under the `^` overflow); its tooltip and menu show the connected tablet (*Galaxy Tab A · USB*) or *Aucune tablette connectée*; *Settings → Personalization → Taskbar → Other system tray icons* names it *Padboard* |
| Tray menu | Left click the icon; right click → *Ouvrir Padboard* / *Quitter Padboard* | Left click and *Ouvrir* show the window; *Quitter* closes Padboard (the tablet loses the PC) |
| First close | Close the window (X) for the first time | A dialog explains Padboard stays in the notification area: *Réduire…* hides the window, the tablet keeps working; *Quitter* quits |
| Next closes | Close the window again | It hides at once, no dialog |
| Second launch | Start Padboard from the Start menu while it runs hidden | The running window comes back (no second Padboard); `%APPDATA%\io.github.hugoambiehl.padboard\padboard.log` keeps the running instance's lines and gains *launched again: showing the window* |
| Startup | *Connexions* → *Démarrage* → turn on, sign out and in | Padboard starts hidden in the notification area and the tablet connects; turned off: it does not start (*Task Manager → Startup apps* lists it while on). The `Padboard` value of `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` is `"<install dir>\padboard-desktop.exe" --hidden`, path in quotes |
| Installed apps | *Settings → Apps → Installed apps* | Padboard's publisher is *Hugo Ambiehl* |

## 6. In-app update (from 0.2)

Tests the real update (download, signature, NSIS install, restart) against
**local test releases**: nothing is published. Two test installers of
different versions are built with the `test-updates` feature (test builds
only: the release workflow refuses it) and served from this PC. They
replace the installed Padboard: note its version first and reinstall the
official one at the end. Everything runs in **test mode**
(`PADBOARD_TEST_DIR`): the real configuration in `%APPDATA%` is never used.

**Before**: quit the running Padboard (tray → *Quitter Padboard*).
Tools: those of 1.C, plus Git Bash with Python 3 (for
`scripts/test-releases.sh`).

### Build the two versions

In `apps\desktop` (PowerShell), on an up-to-date `main`:

```powershell
npm ci
# The versions as config files (JSON on the command line loses its quotes in PowerShell)
Set-Content $env:TEMP\v1.json '{"version":"0.2.0-rc.1"}'
Set-Content $env:TEMP\v2.json '{"version":"0.2.0-rc.2"}'
$common = '--bundles', 'nsis', '--features', 'test-updates', '--config', 'src-tauri/tauri.test-updates.conf.json'
npx tauri build @common --config $env:TEMP\v1.json
Copy-Item ..\..\target\release\bundle\nsis\Padboard_0.2.0-rc.1_x64-setup.exe $env:TEMP\A.exe
npx tauri build @common --config $env:TEMP\v2.json
Copy-Item ..\..\target\release\bundle\nsis\Padboard_0.2.0-rc.2_x64-setup.exe $env:TEMP\B.exe
```

### Serve version B (Git Bash, repository root; keep it open)

```bash
scripts/test-releases.sh add 0.2.0-rc.2 --installer "$TEMP/B.exe"
scripts/test-releases.sh serve      # http://127.0.0.1:8765, prints each request
```

### Install and start version A in test mode (PowerShell)

```powershell
& $env:TEMP\A.exe /S                       # silent: does not start the app
$env:PADBOARD_TEST_DIR = "$env:TEMP\padboard-update-test"
$env:PADBOARD_TEST_RELEASES = 'http://127.0.0.1:8765'
$env:PADBOARD_TEST_UPDATE_KEY = "$PWD\..\..\target\test-releases\test-update.key.pub"
$env:PADBOARD_TEST_APP_VERSION = '0.2.0-rc.1'
& "$env:LOCALAPPDATA\Padboard\padboard-desktop.exe"
```

(If Padboard is installed elsewhere, use the path of its Start menu
shortcut.) The window title must read **Padboard (mode test)**.

### Checks

| Check | Do | Expect |
|---|---|---|
| Found | *Connexions* → *À propos* → *Mises à jour*: turn on **Versions de test**, then *Rechercher* | *La version 0.2.0-rc.2 est disponible.*, the header shows *Mise à jour 0.2.0-rc.2*; the server window shows `GET /api/releases` |
| Edits kept | Rename a button in the editor, then at once *Installer et redémarrer* | A progress bar (passive NSIS), **no UAC prompt**, Padboard comes back by itself, title still *Padboard (mode test)* (else close it at once and note it), the renamed button is there; *Settings → Apps → Installed apps*: Padboard **0.2.0-rc.2**; the test dir's `app.log` holds *installing the update, the app exits* |
| Up to date | Restart version B with the same variables but `PADBOARD_TEST_APP_VERSION = '0.2.0-rc.2'`, *Rechercher* | *Padboard est à jour* (never an older or the same version) |
| Tampered installer | Reinstall A (`& $env:TEMP\A.exe /S`), start it as above; in Git Bash append a byte to the served installer: `printf x >> target/test-releases/releases/download/v0.2.0-rc.2/Padboard_0.2.0-rc.2_x64-setup.exe`; *Rechercher*, *Installer et redémarrer* | *Le fichier téléchargé n’est pas signé pour cette version : mise à jour refusée.*, nothing installed (still 0.2.0-rc.1) |
| Forged version | `scripts/test-releases.sh add 0.2.0-rc.2 --installer "$TEMP/B.exe"` again; then make a fake 9.9.9 that points to it: `mkdir -p target/test-releases/releases/download/v9.9.9 && sed 's/"version": "0.2.0-rc.2"/"version": "9.9.9"/' target/test-releases/releases/download/v0.2.0-rc.2/latest.json > target/test-releases/releases/download/v9.9.9/latest.json && scripts/test-releases.sh remove 0.0.0`; *Rechercher*, *Installer et redémarrer* | 9.9.9 is offered, but installing it is refused (*pas signé pour cette version*): the signature says 0.2.0-rc.2 |
| Stable channel | Turn **Versions de test** off, *Rechercher* | *Padboard est à jour* (the test releases are release candidates; 9.9.9 too is refused: remove it first with `scripts/test-releases.sh remove 9.9.9`) |
| Automatic check off | Turn *Rechercher automatiquement* off, quit, start again, wait 30 s | No `GET /api/releases` in the server window |
| Automatic check on | Turn it on | One `GET /api/releases` at once |

### Afterwards

1. Quit Padboard, stop the server (Ctrl+C), close the PowerShell window
   (the variables go with it).
2. *Settings → Apps* → uninstall Padboard (leave *delete the app data*
   unticked), then install the official installer of the version noted
   before (release page), and check *Settings → Apps* shows it.
3. Delete `%TEMP%\padboard-update-test`, `%TEMP%\A.exe`, `%TEMP%\B.exe`,
   `%TEMP%\v1.json`, `%TEMP%\v2.json` and `target\test-releases`.

## Known Windows limits

- Keys cannot be sent to elevated (administrator) windows.
- Audio states are read every 300 ms (Windows has no simple change feed).
- The installer is not signed yet (SmartScreen warning); SignPath once the project is public.
