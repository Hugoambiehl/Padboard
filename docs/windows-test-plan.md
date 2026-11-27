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

Configuration and the log of the last launch (`padboard.log`; the one
before in `padboard.log.1`, from 0.2.1-rc.2) live in
`%APPDATA%\io.github.hugoambiehl.padboard\`. Padboard keeps the log open
for writing while it runs, so `[IO.File]::ReadAllBytes` (which does not
share writing) fails on it then; read it with shared access:

```powershell
$log = "$env:APPDATA\io.github.hugoambiehl.padboard\padboard.log"
$s = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
$r = New-Object IO.StreamReader($s); $r.ReadToEnd(); $r.Close()
```

## 2. Tablet over USB

1. Get adb: either *Installer adb…* (next table, from 0.2), or
   `winget install Google.PlatformTools` then sign out and back in so
   Padboard sees it in `PATH`, or point Padboard to `adb.exe` (*Connexions*
   → *Appareils USB* → *Choisir l'emplacement d'adb…*). Google's
   Platform-Tools are not free software (Android SDK license).
2. Plug the tablet. Windows may already have a driver for it (Windows
   Update, or installed by another program); Samsung's *Android USB Driver
   for Windows* only if `adb devices` lists nothing (`docs/install-windows.md`, *If adb does not see the tablet*).
3. Accept the *Allow USB debugging* prompt on the tablet (new PC = new key).
4. *Connexions* → *Appareils USB* should say *Prête : port 47321 redirigé*,
   and the tablet connects by itself (pill: *USB*).

### Installer adb (from 0.2)

The button shows only while no adb is found: first remove the winget copy
(`winget uninstall Google.PlatformTools`, then sign out and back in), or
use another Windows account. Normal mode (not test mode: adb is off there).

| Check | Do | Expect |
|---|---|---|
| Offered | *Connexions* → *Appareils USB* | *adb est introuvable. Il ne sert qu'à la connexion USB : le Wi-Fi fonctionne sans.*, the offer text and **Installer adb…** |
| Honest dialog | **Installer adb…** | Says optional; §3.1 and §3.4 summed up; *Padboard n'est pas affilié à Google*; *Lire la licence complète* opens developer.android.com/studio/terms; **Télécharger et installer** disabled until the box is ticked |
| Install | Tick, **Télécharger et installer** | Within seconds: the card shows `…\AppData\Local\io.github.hugoambiehl.padboard\platform-tools\adb.exe` and *Platform-Tools 37.0.3 de Google, téléchargés par Padboard.*; the tablet connects over USB; `padboard.log` holds *the user accepted the Android SDK license* and *platform-tools installed* |
| Signatures | In PowerShell: `Get-AuthenticodeSignature "$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\*.exe"` | *Valid*, signer *CN=Google LLC* for each |
| PATH first | `winget install Google.PlatformTools`, sign out and in, start Padboard | The card shows the winget `adb.exe` (an `adb` in `PATH` wins) |
| Remove | Back to the downloaded copy (uninstall winget's), **Supprimer adb** | The folder is gone, the offer is back, the USB connection stops; no error; `padboard.log`: *our adb server stopped* |
| Remove, another program's server | Install again; stop the server (`…\platform-tools\adb.exe kill-server`), start another adb's (Touch Portal's, or any adb in another folder: `<that adb> start-server`), wait for the card to be ready, **Supprimer adb** | The folder is gone; the other server still runs: `<that adb> devices` answers at once, without *daemon started*; `padboard.log`: *adb server started by another program: left running* with its path (from 0.2.0-rc.2; rc.1 stopped it) |
| Licences | *À propos* → *Licences* | *Téléchargé à la demande, non inclus dans Padboard: Android SDK Platform-Tools 37.0.3 · Google LLC* |
| Pinned archive (from 0.4) | After *Install* | No orange line under the path; `padboard.log` has no *accepted from Google's index*; no `padboard-from-index.json` in the folder |

A republished archive accepted from Google's index (#18, from 0.4) cannot
be produced on demand: the unit tests of `platform_tools` cover it (index
read, same version / file name / size / SHA-1 required, signatures still
checked). If it ever happens for real: orange line *Archive republiée par
Google…* under the path, `padboard.log` at WARN *NOT the pinned archive,
accepted from Google's index* with both SHA-256s and the SHA-1, and
`padboard-from-index.json` next to `adb.exe`; then pin the new archive
(`scripts/platform-tools-pin.sh`) in the next release.

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

Tests the real update (download, signature, installer shown in the
Explorer, install run by hand, restart) against
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
| Shown, not started | Rename a button in the editor, then *Télécharger* | *La version 0.2.0-rc.2 est téléchargée et vérifiée.*, *Ferme Padboard, puis lance ce fichier :* and the path (test mode: `%TEMP%\padboard-update-test\config\downloads\Padboard_0.2.0-rc.2_x64-setup.exe`); the **Explorer opens with that file selected**; Padboard keeps running, **no installer starts by itself**; `app.log`: *installer downloaded and checked*, *the installer is not started by Padboard* |
| Show again | Close the Explorer, *Afficher le fichier* | The Explorer opens again on the file |
| Edits kept | *Quitter Padboard* (in the card), then double-click the file | The installer's **wizard**, **no UAC prompt**; follow it to the end and leave *Run Padboard* ticked: Padboard starts, title still *Padboard (mode test)* (else close it at once and note it), the renamed button is there; *Settings → Apps → Installed apps*: Padboard **0.2.0-rc.2** |
| Up to date | Restart version B with the same variables but `PADBOARD_TEST_APP_VERSION = '0.2.0-rc.2'`, *Rechercher* | *Padboard est à jour* (never an older or the same version) |
| Tampered installer | Reinstall A (`& $env:TEMP\A.exe /S`), delete the test dir's `config\downloads`, start A as above; in Git Bash append a byte to the served installer: `printf x >> target/test-releases/releases/download/v0.2.0-rc.2/Padboard_0.2.0-rc.2_x64-setup.exe`; *Rechercher*, *Télécharger* | *Le fichier téléchargé n’est pas signé pour cette version : mise à jour refusée.*, no file in `config\downloads`, no Explorer |
| Forged version | `scripts/test-releases.sh add 0.2.0-rc.2 --installer "$TEMP/B.exe"` again; then make a fake 9.9.9 from its files: `cp -r target/test-releases/releases/download/v0.2.0-rc.2 target/test-releases/releases/download/v9.9.9 && cd target/test-releases/releases/download/v9.9.9 && for f in Padboard_0.2.0-rc.2_x64-setup.exe*; do mv "$f" "${f/0.2.0-rc.2/9.9.9}"; done && cd - && scripts/test-releases.sh remove 0.0.0`; *Rechercher*, *Télécharger* | 9.9.9 is offered, but the download is refused (*pas signé pour cette version*): the signatures say 0.2.0-rc.2 and another file name |
| Stable channel | Turn **Versions de test** off, *Rechercher* | *Padboard est à jour* (the test releases are release candidates; 9.9.9 too is refused: remove it first with `scripts/test-releases.sh remove 9.9.9`) |
| Automatic check off | Turn *Rechercher automatiquement* off, quit, start again, wait 30 s | No `GET /api/releases` in the server window |
| Automatic check on | Turn it on | One `GET /api/releases` at once |
| Refusal kept | Tampered installer as above, then *Télécharger* within 10 s of the launch | The refusal stays shown after the automatic check (10 s after launch) |
| Still ready | Reinstall A, start it, *Télécharger*, then *Quitter Padboard* **without** running the file; start A again (same variables) | *La version 0.2.0-rc.2 est téléchargée et vérifiée.* with the same file and *Afficher le fichier* (no Explorer opened by itself at launch); it stays after the automatic check |
| Not installed | Same, but delete the downloaded file before starting A again | *L’installation de la version 0.2.0-rc.2 n’a pas abouti.*, with *Que faire ?* (opens *If your antivirus blocks Padboard*) and *Voir la version*; the next launch no longer shows it |
| Network down at launch | Stop the server (Ctrl+C), start A, wait 20 s; start the server again | No error shown (*Pas encore vérifié.*); `app.log`: *automatic update check failed (not shown)* then *next update check in 5 min*; 5 min later one `GET /api/releases` and the update is offered |

### Real update with an antivirus (each release candidate)

The checks above use a local test server and installers built on the PC.
Before a release, do one real update on a Windows PC set up like a user's,
with **Bitdefender active** (real-time protection and *Advanced Threat
Control* on, no exception for Padboard):

1. Install release candidate N from its GitHub release (installer
   downloaded with the browser), turn on *Versions de test*.
2. Publish release candidate N+1 (for v0.2.0: rc.1 then a minimal rc.2).
3. In Padboard N: *Rechercher*, *Télécharger*: the Explorer opens on the
   installer in *Downloads*; *Quitter Padboard*, run the file, follow the
   wizard. (Up to 0.2.0-rc.2, Padboard started the installer itself: never
   use *Installer* in those versions, see section 8.) The local test
   releases above cannot replace this step on such a PC: the silent
   `A.exe /S` is itself taken by Bitdefender ATC (section 9.2).
4. Expect: no detection, Padboard N+1 starts, configuration kept. Note any
   Bitdefender event (name, process chain) and report it as a false
   positive (`docs/install-windows.md`).

### Afterwards

1. Quit Padboard, stop the server (Ctrl+C), close the PowerShell window
   (the variables go with it).
2. *Settings → Apps* → uninstall Padboard (leave *delete the app data*
   unticked), then install the official installer of the version noted
   before (release page), and check *Settings → Apps* shows it.
3. Delete `%TEMP%\padboard-update-test`, `%TEMP%\A.exe`, `%TEMP%\B.exe`,
   `%TEMP%\v1.json`, `%TEMP%\v2.json` and `target\test-releases`.

## 7. Release candidate 0.2.0-rc.1 (Bitdefender active)

On Hugo's Windows PC, after `git pull`. Report: one table per step
(check / result / remark), the screenshots taken, and every Bitdefender
event copied as shown (detection name, process, path). **Never work around
anything**: no Bitdefender exclusion, no protection turned off; on a
detection, stop, note it and ask Hugo.

### 7.0 Before starting

- rc.1 is a **draft** pre-release: its files only download with `gh`
  logged in to Hugo's account. So the tablet update from the PC and the
  update check **cannot see rc.1** while it is a draft: not a defect.
- Note: the installed Padboard version (*Settings → Apps*); Bitdefender's
  state (real-time protection and *Advanced Threat Control* **on**, no
  exception for Padboard: screenshot); `where.exe adb`; the default output
  and microphone volume and mute states (restore them exactly at the end).
- Never press a keyboard shortcut button of the tablet during these tests.

### 7.1 Download and check (PowerShell, new folder)

```powershell
mkdir $env:TEMP\padboard-rc1; cd $env:TEMP\padboard-rc1
gh release download v0.2.0-rc.1 --repo Hugoambiehl/Padboard -p "Padboard_0.2.0-rc.1_x64-setup.exe" -p "SHA256SUMS" -p "SHA256SUMS.sig"
Get-FileHash .\Padboard_0.2.0-rc.1_x64-setup.exe
Select-String "x64-setup.exe$" .\SHA256SUMS
```

Expect the same SHA-256 on both lines:
`aa792befcec7a285341d39767cc1fff8e1cbd4fb723585cc6e8820d0e2ea070f`. Note
any Bitdefender reaction to the download.

### 7.2 Install by hand, Bitdefender active

Quit Padboard if it runs (notification area icon → *Quit Padboard*), then
run the installer **from the Explorer** (double click), as a user would.
SmartScreen: *More info* → *Run anyway* (unsigned installer, expected).
Follow the wizard, leave *Run Padboard* ticked.

| Check | Expect |
|---|---|
| Bitdefender during and after the install, and 5 min after the first launch | No detection, no quarantine (else: screenshot of the notification and of Bitdefender's history, exact detection name, and stop) |
| UAC | No prompt (per-user install) |
| *Settings → Apps* | One Padboard entry, **0.2.0-rc.1** (not two installs side by side) |
| *Connexions* → *À propos* | *Padboard version 0.2.0-rc.1*; previous configuration kept (profiles, buttons) |
| *À propos* → *Licences* | Padboard first, then *Téléchargé à la demande, non inclus dans Padboard* (Android SDK Platform-Tools 37.0.3 · Google LLC) |
| *Mises à jour*, *Versions de test* on, *Rechercher* | *Padboard est à jour* (rc.1 is the newest visible: expected) |

### 7.3 "Installer adb", Bitdefender active

No adb may be found first: if `where.exe adb` finds winget's,
`winget uninstall Google.PlatformTools`, sign out and in, start Padboard,
and check the *Appareils USB* card says *adb est introuvable…*.

Run every row of **"Installer adb (from 0.2)"** (section 2) in order, *PATH
first* last. Also:

| Check | Expect |
|---|---|
| Bitdefender during the download and the first run of `adb.exe` | No detection (else: screenshot, exact name, stop) |
| `Get-AuthenticodeSignature "$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\*.exe","$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\*.dll"` | `Valid`, signer `CN=Google LLC` for adb.exe, fastboot.exe, AdbWinApi.dll, AdbWinUsbApi.dll |
| `padboard.log` (`%APPDATA%\io.github.hugoambiehl.padboard\`) | *the user accepted the Android SDK license*, then *platform-tools installed*; no error |
| *Supprimer adb*, then *Installer adb…* again | Removed then installed again without error; another program's adb server (Touch Portal) keeps running (from 0.2.0-rc.2) |

*PATH first*: reinstall winget's copy if Hugo wants it
(`winget install Google.PlatformTools`, sign out and in) and check the card
then shows winget's adb.

### 7.4 Tablet over USB

The tablet (SM-T580, LineageOS 19.1) is **in kiosk mode** and on
**0.2.0-rc.1** (updated from the Linux PC before). Do not leave kiosk mode.

1. Plug it in; accept *Allow USB debugging?* if asked (new PC, new key).
2. Section 2, steps 2 to 4.

| Check | Expect |
|---|---|
| Driver | Recognized. Note which driver Windows uses (Device Manager → the tablet → *Driver*: Samsung's or Microsoft's). If `adb devices` is empty: *If adb does not see the tablet* in `docs/install-windows.md`, note what was needed |
| *Appareils USB* | *Prête : port 47321 redirigé*, *Padboard 0.2.0-rc.1*, kiosk active; no *Mettre à jour* button (same version) |
| Tablet | The grid of the profile shown on the PC, USB pill; stays locked in kiosk mode |
| A few non-keyboard buttons | Media, volume, microphone: the action happens and the state shows on the tablet (restore the sound exactly afterwards) |
| Unplug, plug again | *Connexion au PC perdue* after ~1 s, reconnects by itself when plugged again |

### 7.5 Afterwards

Leave rc.1 installed and Bitdefender on; restore the sound noted in 7.0;
delete `%TEMP%\padboard-rc1`; send the report. The real update rc.1 → rc.2
with Bitdefender active: section 8.

## 8. Release candidate 0.2.0-rc.2: real update from rc.1 (Bitdefender active)

Same PC and same rules as section 7 (report tables, every Bitdefender
event copied, never work around anything). Padboard **0.2.0-rc.1**
installed from its release (section 7), Bitdefender on with no exception.
rc.2 is **published** as a pre-release (a draft is invisible to the
updater and to the tablet update).

### 8.0 Before starting

- Note: the installed version (*Settings → Apps*: 0.2.0-rc.1); Bitdefender's
  state (screenshot); `where.exe adb`; whether Touch Portal runs; the
  default output and microphone volume and mute states (restore them
  exactly at the end).
- Copy `%APPDATA%\io.github.hugoambiehl.padboard\padboard.log` aside (it
  starts over at each launch).
- Never press a keyboard shortcut button of the tablet.

### 8.1 Update the PC from Padboard

| Check | Do | Expect |
|---|---|---|
| Found | *Connexions* → *À propos* → *Mises à jour*, *Versions de test* on, *Rechercher* | *0.2.0-rc.2* available (header badge too) |
| Install | *Installer* | Download, signature checked, then Padboard closes and the installer opens **as a wizard** (not silently: expected while it is unsigned); SmartScreen may ask (*More info* → *Run anyway*); follow it, *Run Padboard* ticked |
| Bitdefender | During the download, the install and 5 min after the first launch | No detection, no quarantine, no *Advanced Threat Control* event (else: screenshot, exact name, process chain, stop) |
| Version | *À propos*; *Settings → Apps* | *0.2.0-rc.2*; one Padboard entry (not two side by side) |
| Kept | Profiles, buttons, paired tablets, Wi-Fi setting, *Versions de test*, adb location | Unchanged |
| No failure notice | *Mises à jour* | No *L’installation de la version … n’a pas abouti* (`update-pending.json` cleared) |

### 8.2 "Supprimer adb" next to Touch Portal

Touch Portal running (its adb server up). If Padboard's downloaded copy
was removed, *Installer adb…* first.

| Check | Do | Expect |
|---|---|---|
| Remove | *Appareils USB* → **Supprimer adb** | The downloaded copy is gone; `padboard.log`: *adb server started by another program: left running* with Touch Portal's `adb.exe` path |
| Touch Portal | Right after, in Touch Portal | Its Android connection still works (its server was not stopped) |
| Put back | *Installer adb…* again if wanted | Installed without error |

Report also the section 2 row *Remove, another program's server* if
Touch Portal is not installed.

### 8.3 Update the tablet from the PC

The tablet on **0.2.0-rc.1**, plugged in over USB. First turn **kiosk mode
on** again (*Appareils USB* → *Activer le mode kiosque*): rc.2's kiosk
exit logging then follows it from the update on.

| Check | Do | Expect |
|---|---|---|
| Offered | *Appareils USB* | *Padboard 0.2.0-rc.1* and **Mettre à jour vers 0.2.0-rc.2** |
| Update | **Mettre à jour vers 0.2.0-rc.2** | Within a minute: *Padboard 0.2.0-rc.2*; the tablet shows Padboard again by itself (no launcher in between, kiosk) and reconnects |
| Still kiosk | `adb shell dpm list-owners` | Padboard listed; status bar hidden, locked in |
| Bitdefender | During the APK download (`%APPDATA%\…\updates\`) and `adb install` | No detection |
| Logs | `adb logcat -d -s Kiosk`; `padboard.log` | No *leaving kiosk mode*; no *the tablet left kiosk mode* |

Then the tablet checks of `docs/tablet-test-plan.md` (section 0.2.0-rc.2)
can run with this PC.

### 8.4 Afterwards

Leave rc.2 installed and Bitdefender on; restore the sound noted in 8.0;
send the report with both `padboard.log` files (before and after the
update).

### 8.5 Results (reported 2026-11-16): the in-app install fails

Bitdefender active, *Advanced Threat Defense* on, no exception for
Padboard.

| Check | Result | Remark |
|---|---|---|
| 8.1 Found | OK | *Versions de test* + *Rechercher*: rc.2 offered, header badge |
| 8.1 Install | **FAIL** | rc.1 started the checked installer (SHA-256 `767172bd…151e`) at 23:00:09; 28 s later Bitdefender ATC classified it `Atc4.Detection` and quarantined 9 files: the installer, rc.1's `padboard-desktop.exe`, `preferences.json`, `update-pending.json`, 3 Microsoft `IdentityCache` files and WebView2's `Crashpad\settings.dat`. The wizard never showed (even in `basicUi`) |
| Manual install right after | **FAIL** ×3 | Double-click on the same installer: NSIS *Erreur lors de l'ouverture du fichier en écriture : …\padboard-desktop.exe*, no file created, no handle, no notification, no quarantine. A PowerShell write test in the same folder passed at the same time: the installer was blocked, not the folder |
| After a PC restart | OK | The same installer, by hand, installed with no exception; profiles and preferences kept |
| First launch after the restart | Issue | The update check failed (*error sending request*, network not ready yet) and the next one was 24 h later |
| VirusTotal (rc.2 installer) | 1/71 | SecureAge only; Bitdefender does not detect it statically, only by behaviour (ATC) when Padboard starts it |

Conclusion: a Bitdefender user could lose Padboard without any message.
From rc.3 Padboard never starts an unsigned installer: it downloads and
checks it, then shows it in the Explorer (section 9); a failed automatic
check is retried after 5 then 30 minutes, silently. 8.2 and 8.3 were not
reported in this round: run them in section 9.

## 9. Release candidate 0.2.0-rc.3: update from rc.2 (Bitdefender active)

Same PC and rules as sections 7 and 8. rc.3 is **published** as a
pre-release. **rc.2 still has the old *Installer* button, which starts the
installer itself (the failure of 8.5): never click it.** So the update to
rc.3 is made by hand once, and the new path (Explorer) is checked from rc.3
itself, with local test releases (section 6) until rc.4 exists.

### 9.0 Before starting

As 8.0 (version: 0.2.0-rc.2 or whatever is installed; Bitdefender state;
`where.exe adb`; Touch Portal; sound; copy `padboard.log` aside). If 8.5
left Padboard broken, restart the PC first.

### 9.1 rc.2 → rc.3 by hand

| Check | Do | Expect |
|---|---|---|
| Found | In rc.2: *Mises à jour*, *Versions de test* on, *Rechercher* | *0.2.0-rc.3* available (badge). **Do not click *Installer*** |
| Download | *Voir la version*: download `Padboard_0.2.0-rc.3_x64-setup.exe` with the browser; `Get-FileHash` | Same SHA-256 as in the release's `SHA256SUMS` |
| Install | Tray → *Quitter Padboard*, then double-click the installer | Wizard, no detection; Padboard 0.2.0-rc.3 starts; profiles, tablets, preferences kept; one entry in *Settings → Apps* |
| Bitdefender | During the install and 5 min after | No detection, no quarantine, no ATC event |
| Silent retry | Restart the PC; Padboard starts with the session if that is on (else start it at once) | No update error shown even if the network comes up late; `padboard.log`: either *no update* / *update available*, or *automatic update check failed (not shown)* followed by *next update check in 5 min* |

### 9.2 The new update path, from rc.3: postponed to 0.2.0

**Not run with local test releases** (decided 2026-11-16): with
Bitdefender active, section 6's `A.exe /S` started from PowerShell was
taken by ATC (`Atc4.Detection`, 19:50): the test installer, the installed
`padboard-desktop.exe`, `uninstall.exe` and both shortcuts quarantined,
the whole process chain killed, a firewall block rule added for each
executable. A locally built, unsigned installer run silently by a script
is the pattern ATC takes; section 6 stays for PCs without such an
antivirus.

Run instead **when 0.2.0 is published**, with the real update from the
official **rc.4** (section 10 installs it; Bitdefender active, no
exception, *Versions de test* on):

| Check | Do | Expect |
|---|---|---|
| Found | *Rechercher* | *0.2.0* available (badge) |
| Not started by Padboard | *Télécharger* | Downloaded and checked; the Explorer opens on `Padboard_0.2.0_x64-setup.exe` in *Downloads*; no installer process starts until you run it; **no Bitdefender event** during the download and while Padboard runs |
| Show again | Close the Explorer, *Afficher le fichier* | The Explorer opens again on the file |
| Run by the user | *Quitter Padboard* (in the card), double-click the file; previous version found: **Désinstaller avant d'installer** | Wizard, no detection; rc.4 is uninstalled as an update (no confirmation, no *Supprimer les données de l'application*; if it shows anyway, leave it **unticked** and report it); 0.2.0 starts; profiles, preferences, *Versions de test* kept; one entry in *Settings → Apps* |
| No failure notice | *Mises à jour* | No *L’installation de la version … n’a pas abouti* |
| Firewall | Bitdefender's firewall rules | No new rule for `padboard-desktop.exe` |

### 9.3 USB, adb and tablet

8.2 ("Supprimer adb" next to Touch Portal) and 8.3 (tablet update from
the PC, kiosk on first, to **0.2.0-rc.3**), unchanged.

### 9.4 Afterwards

Reinstall the official rc.3 if section 6 replaced it (*Afterwards* of
section 6); restore the sound; send the report with the `padboard.log`
files.

### 9.5 Results (reported 2026-11-16)

| Check | Result | Remark |
|---|---|---|
| 9.1 rc.2 → rc.3 by hand | OK | Tray, silent retry after a restart, single instance OK |
| 9.2 | Postponed | To the real update to 0.2.0, from rc.4 (see 9.2): done, see 9.6 |
| 9.3 (8.2, 8.3) | OK | "Supprimer adb" next to Touch Portal; kiosk on, then tablet updated from the PC in 4 s; the new exit gesture (bottom-left corner) tried by finger |

Found during this round, fixed or documented in rc.4 (section 10):

1. After an ATC detection, Bitdefender silently adds **firewall rules**
   blocking `%LOCALAPPDATA%\Padboard\padboard-desktop.exe`: by path, so
   they survive a reinstall, and the update check fails (*error sending
   request*) with nothing shown. Documented (`install-windows.md`).
2. The **autostart `Run` value** (`HKCU\…\Run\Padboard`) was gone after
   the remediation. Tauri's uninstaller also removes it when the next
   installer runs it as a plain uninstall (*Désinstaller avant
   d'installer*). rc.4 records the choice and writes the value again at
   launch.
3. During an update by double-click, the old version's uninstall step
   offered **Supprimer les données de l'application**: easy to tick by
   mistake, and the whole configuration is lost (PC identity, TLS key,
   pairings, profiles). From rc.4 the installer runs it as an update
   (`/UPDATE /P`, `src-tauri/windows/installer.nsi`).
4. The PC described the **new exit gesture** while the connected tablet
   still had the old version. rc.4 describes the gesture of the version
   installed on the tablet.
5. The installer showed NSIS's **default images** (the blue computer).
   rc.4 has Padboard's (`scripts/icons.mjs`).

### 9.6 Results of 9.2: rc.4 → 0.2.0 (reported 2026-11-17)

Official rc.4 → published 0.2.0, Bitdefender active, no exception,
*Versions de test* on.

| Check | Result | Remark |
|---|---|---|
| Found | OK | *Rechercher*: *update available version=0.2.0* |
| Not started by Padboard | OK | *Télécharger*: `Padboard_0.2.0_x64-setup.exe` in *Downloads*, *installer downloaded and checked*, then *not started by Padboard* (`no trusted publisher in this build`); SHA-256 `599289e6…4aaf4ed08` = `SHA256SUMS`; no installer process until the double-click; no Bitdefender event |
| Show again | OK | *Afficher le fichier*: the Explorer opened again on *Downloads*, the file selected |
| Run by the user | OK | *Quitter Padboard* in the card, double-click: Padboard's images; *Déjà installé* page, *Désinstaller avant d'installer*; no confirmation page, no *Supprimer les données de l'application*; install folder unchanged; 0.2.0 started (*update installed version=0.2.0*), tablet reconnected over USB in 4 s. `identity`, `profiles`, `paired-devices`, `obs`, `tls-key`, `preferences` unchanged byte for byte (*Versions de test* kept); one entry in *Settings → Apps* (0.2.0); `Run` value and `StartupApproved` (`02…`) unchanged (*entry refreshed*); Start menu and desktop shortcuts kept |
| No failure notice | OK | No *… n’a pas abouti* in *Mises à jour* |
| Firewall | OK | No ATC detection, no quarantine, no ATC / ctc file during the install and 5 min after (21:38 → 21:44). One new rule, before the install: a **temporary allow** for the running rc.4 (`--hidden`), added when it downloaded the installer (as in 10.5); nothing during the install or for 0.2.0 |
| Tablet (8.3) | OK | Kiosk on, USB card: 20004 → 20099 (0.2.0) in 5 s; still device owner, lock task `LOCKED`, grid shown; `<config>/updates` keeps only the 0.2.0 APK |
| Sound | OK | Same as before the test, nothing to restore |

Remarks:

1. During an update the installer still shows the folder page
   (*Choisissez le dossier d'installation*, prefilled with the current
   one). A different folder would install beside the old path: the `Run`
   value is written again at launch, but shortcuts, a taskbar pin and
   Bitdefender's rules by path keep pointing to the old one.
2. The *Déja installé.* / *La version ancien* typos of Tauri's NSIS
   language file are still there (10.5, remark 1).

## 10. Release candidate 0.2.0-rc.4: update from rc.3 (Bitdefender active)

Same PC and rules as sections 7 to 9. rc.4 is **published** as a
pre-release. The update is the new path of rc.3: Padboard downloads and
shows the installer, you run it.

### 10.0 Before starting

As 9.0, plus:

- In Bitdefender (*Protection* → *Firewall* → *Rules*): note then delete
  the rules for `padboard-desktop.exe` left by section 9.2's detection
  (`install-windows.md`, *If your antivirus blocks Padboard*).
- Note the autostart state, to restore it at the end:
  `Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run | Select-Object Padboard`,
  and Padboard's line in *Task Manager* → *Startup apps*.
- In rc.3: *Connexions* → *Démarrage*, *Lancer Padboard à l'ouverture de
  session* **on** (for 10.1's checks).

### 10.1 rc.3 → rc.4 (a: no data deletion offered)

| Check | Do | Expect |
|---|---|---|
| Found | In rc.3: *Versions de test* on, *Rechercher* | *0.2.0-rc.4* available (badge); no *error sending request* in `padboard.log` once the firewall rules are gone |
| Shown | *Télécharger* | The Explorer opens on `Padboard_0.2.0-rc.4_x64-setup.exe` in *Downloads*; `Get-FileHash` = `SHA256SUMS`; no Bitdefender event |
| Images (d) | Double-click the file **while rc.3 still runs** | Welcome and finish pages: Padboard's dark sidebar (violet icon, coloured tiles); other pages: Padboard's header. No blue computer |
| Padboard running | *Désinstaller avant d'installer*, *Suivant* | The installer asks to close Padboard (OK / Cancel). *Annuler*: back on the same page, nothing uninstalled. Then *Suivant* again, *OK*: Padboard closes |
| No deletion offered | (continued) | rc.3's uninstaller **shows no confirmation page and no *Supprimer les données de l'application***: only a short progress window that closes by itself, then the install. If the checkbox shows anyway: leave it **unticked**, finish, and report a) as failed |
| Kept | rc.4 starts | Profiles, preferences, *Versions de test*, paired tablets kept (the tablet reconnects without a PIN: same PC identity and key); Start menu and desktop shortcuts and a taskbar pin kept; one entry in *Settings → Apps*; the `Run` value still there |
| Bitdefender (`/P` uninstall) | Bitdefender active, update rc.3 → rc.4 by double-click | **No ATC detection, no quarantine, no new firewall rule during the install and 5 min after** |

The installer now runs the old `uninstall.exe` with `/P` (no window), a
child process started without a visible window: close to the pattern ATC
took in 9.2 (`A.exe /S` started by a script). If ATC reacts: report it,
restart the PC, reinstall rc.4 by hand (*If your antivirus blocks
Padboard*, `install-windows.md`), and the fallback for rc.5 is to drop
`/P` and keep `/UPDATE` in `src-tauri/windows/installer.nsi`. The
uninstaller's window then shows; its data is still never deleted
(`/UPDATE` skips it). Uninstallers from rc.4 on hide the *Supprimer les
données de l'application* checkbox in that mode, but those of 0.1.0 to
rc.3 (Tauri's own) still **show it, without effect**. The template's
comments and its test change with it.

### 10.2 Start with the session (b)

Quit Padboard (tray → *Quitter Padboard*) before each registry change.

| Check | Do | Expect |
|---|---|---|
| Choice recorded | First launch of rc.4 (10.1) | `padboard.log`: *start with the session: choice recorded from the entry* `on=true` (once; later launches: *entry refreshed*) |
| Written again | `Remove-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name Padboard`, start Padboard | `padboard.log`: *the entry was missing (removed outside Padboard: uninstaller, security software, Task Manager…), written again* (up to 0.2.x: *the entry had disappeared (removed by another program)*); the value is back: `"…\padboard-desktop.exe" --hidden`; *Démarrage* on |
| Task Manager respected | *Task Manager* → *Startup apps* → Padboard **Disabled**; quit; remove the `Run` value again; start Padboard | The value is written again (log as above), but Padboard **stays Disabled** in the Task Manager; restart the PC: Padboard does not start. Then set it back to **Enabled** |
| Turned off | *Démarrage* off, quit, start Padboard | No `Run` value, nothing written again |
| Restart | *Démarrage* on, restart the PC | Padboard starts hidden in the tray |

### 10.3 Tablet (c)

| Check | Do | Expect |
|---|---|---|
| Gesture text | Tablet in kiosk mode on rc.3, USB card | *… coin en bas à gauche …* (rc.2 and later). With a tablet still on 0.1.0 or rc.1, if one is at hand: *… coin en haut à droite … (sa version actuelle …)* |
| Update | 8.3 to **0.2.0-rc.4** | Updated from the PC; the text is unchanged (bottom-left) |

### 10.4 Afterwards

Restore the autostart state noted in 10.0 (and the Task Manager line),
the sound, then send the report with the `padboard.log` files. rc.4 stays
installed: 9.2 runs from it when 0.2.0 is published.

### 10.5 Results (reported 2026-11-17)

| Check | Result | Remark |
|---|---|---|
| 10.0 | OK | No firewall rule left for `padboard-desktop.exe`, quarantine empty, *Démarrage* already on |
| Found | OK | rc.3 offered 0.2.0-rc.4 at launch, no request error |
| Shown | OK | *Télécharger*: the Explorer opened on the file selected in *Downloads*, not started (`no trusted publisher in this build`); SHA-256 = `SHA256SUMS`; no Bitdefender event |
| Images (d) | OK | Padboard's sidebar and header |
| Padboard running | OK | Asked to close Padboard; *OK* closed rc.3. *Annuler* was checked afterwards by running the rc.4 installer again over rc.4: the same-version page (*Ajouter/Réinstaller* · *Désinstaller Padboard*) takes the same path; *Désinstaller Padboard*, *Suivant*, *Annuler*: back on that page, Padboard still running, installed files unchanged, no uninstaller started |
| No deletion offered (a) | OK | No confirmation page, no checkbox |
| Kept | OK | `identity`, `profiles`, `paired-devices`, `obs`, `tls-key` unchanged byte for byte; `preferences.json` only gained `"autostart": true`; tablet reconnected without a PIN; Start menu and desktop shortcuts kept; one entry in *Settings → Apps* (0.2.0-rc.4); `Run` value kept |
| Bitdefender (`/P` uninstall) | OK | No ATC detection, no quarantine, no ATC / ctc file from 17:54 to 18:01 and later. One new firewall rule: a **temporary allow** for the running rc.3 (`--hidden`), added when it downloaded the installer; nothing for the installer or the uninstaller |
| 10.2 Choice recorded | OK | `choice recorded from the entry on=true` at rc.4's first launch |
| 10.2 Written again | OK | Value removed, Padboard started: written again, same content, `StartupApproved` untouched (`02…`) |
| 10.2 Task Manager respected | OK | Disabled (`StartupApproved` `03…`), value removed, Padboard started: value written again, `StartupApproved` unchanged; after a restart Padboard did not start. Enabled again |
| 10.2 Turned off | OK | *Démarrage* off: value removed; quit and started again: nothing written |
| 10.2 Restart | OK | Started hidden in the tray, tablet reconnected over USB |
| 10.3 Gesture text (c) | OK | Tablet on rc.3, in rc.3 then in rc.4: *… coin en bas à gauche …* (no tablet below rc.2 at hand) |
| 10.3 Update | OK | 20003 → 20004 over USB in about 3 s; kiosk kept (device owner, lock task); same text |
| 10.4 | OK | `Run` value, `StartupApproved` and sound back exactly as noted in 10.0 |

Remarks:

1. The French texts of the *Déjà installé* page come from Tauri's NSIS
   language file and have two mistakes: *Déja installé.* and *La version
   ancien de Padboard est installée…*. Not from our template.
2. The `/P` uninstaller was too short-lived for the watcher (process list
   every 0.7 s) to see it.
3. Padboard uses Touch Portal's `adb` found in `PATH` (expected order:
   setting, `PATH`, SDK, our copy).

## 11. Release candidate 0.2.1-rc.2: update from 0.2.0 (Bitdefender active)

Same PC and rules as section 10, 0.2.0 installed, Bitdefender active with
no exception. 0.2.1 fixes: the folder page during an update (#12), the
installer's French texts (#10), the OBS scene list (#13), and OBS
freezing while it starts when Padboard runs (#14, found in rc.1, which
rc.2 replaces).

### 11.0 Before starting

As 10.0. Copy `%APPDATA%\io.github.hugoambiehl.padboard` somewhere safe
(11.4 deletes it). In 0.2.0: *Versions de test* **on**.

### 11.1 0.2.0 → 0.2.1-rc.2 by the new path

| Check | Do | Expect |
|---|---|---|
| Shown | *Rechercher*, *Télécharger* | The Explorer opens on `Padboard_0.2.1-rc.2_x64-setup.exe` in *Downloads*; `Get-FileHash` = `SHA256SUMS`; no Bitdefender event |
| French texts | *Quitter Padboard*, double-click the file | Page title **Déjà installé.**, subtitle **Choisissez comment installer Padboard.**, text **La version ancienne de Padboard est installée…**; no *déja*, *continer* |
| No folder page | *Désinstaller avant d'installer* (default), *Suivant* | Straight to the installation: **no *Choisissez le dossier d'installation* page**; no *Supprimer les données de l'application* |
| Kept | 0.2.1-rc.2 starts | Same folder (`%LOCALAPPDATA%\Padboard`); profiles, preferences, paired tablets kept; shortcuts, taskbar pin and the `Run` value kept; one entry in *Settings → Apps* |
| Other choice | Run the same installer again: **Ajouter/Réinstaller un composant**, *Suivant* | No folder page either; configuration kept |
| Bitdefender | Whole section | **No ATC detection, no quarantine, no new firewall rule** |

**Result (2026-11-19)**: passed. The update was made from rc.1
(section 10), installed in `D:\Programme Files\Padboard`: *Rechercher*,
*Télécharger*, SHA-256 checked; rc.2 stayed in that folder, no
`%LOCALAPPDATA%\Padboard`; `Run`, `StartupApproved`, install key and
shortcuts unchanged, one entry in *Settings → Apps*; every configuration
file's hash unchanged (`update-pending.json` removed);
`padboard.log.1` present. *Other choice* passed too (same checks, no
folder page, no data checkbox). Bitdefender: no detection, quarantine
empty, during and 5 min after; one new firewall rule, automatic and
temporary (*allow*), for rc.1's `padboard-desktop.exe` while it
downloaded rc.2 (not seen from 0.2.0 to rc.1), and one for GoPro
Webcam, unrelated.

### 11.2 OBS scene list (#13)

| Check | Do | Expect |
|---|---|---|
| OBS after Padboard | OBS closed; start Padboard, open the editor on an OBS *Changer de scène* button; then start OBS | Once *Connexions* says OBS is connected, the *Scène* list shows OBS's scenes, in OBS's order, **without reopening anything** |
| Created / renamed | In OBS: add a scene, rename another, remove a third | The list follows each change within a second |
| Collection | In OBS: *Collection de scènes* → another one | The list shows the new collection's scenes |

**Result (2026-11-19, rc.2)**: *OBS after Padboard* passed: free text
while OBS was closed, then the 8 scenes without reopening anything, once
Padboard connected (OBS's window frozen, obs-websocket answering, see
11.3). *Created / renamed* postponed while OBS freezes at start (done in
OBS's window); *Collection* skipped.

### 11.3 OBS starting while Padboard runs (#14)

OBS 33 with obs-websocket on, set up in Padboard (*Connexions* → OBS),
GoPro Webcam and the rest as usual. In rc.1, OBS froze while starting
(`AppHangB1`); rc.2 asks nothing while OBS loads, then waits 10 s after
its scenes are loaded before connecting for real.

| Check | Do | Expect |
|---|---|---|
| 5 starts | Padboard running, OBS closed. Start OBS, wait until it is fully open and *Connexions* says OBS is connected, use one OBS button from the tablet, close OBS; **5 times** | **OBS never freezes** (no *ne répond pas*, no `AppHangB1` in *Observateur d'événements → Journaux Windows → Application* at those times); each time OBS is connected about 10–20 s after it opens, the buttons follow it |
| Log | `padboard.log`, for one start | *OBS not reachable* (once), then *connecting to OBS at ws://… (only checking that it has loaded)*, *OBS is still loading (207 not ready)* (maybe a few times), *OBS has loaded its scenes: waiting 10 s for it to finish starting*, *connecting to OBS at ws://…*, *connected to OBS*, the *OBS request* / *OBS answered* lines, *OBS request GetInputMute (one batch)* then *OBS answered GetInputMute* with the input count; no *OBS refused an initial request* |
| Previous log | Quit and start Padboard again | `padboard.log.1` next to `padboard.log` holds the previous launch's log |

If OBS freezes: note the time, do not restart Padboard first (its log
would move to `padboard.log.1`, which is kept, but a second restart would
erase it), copy both `padboard.log` files and the OBS log of that start
(*Aide → Fichiers journaux → Afficher les fichiers journaux*).

**Result (2026-11-19, rc.2): OBS froze, without Padboard too.** The first
start froze (`AppHangB1`) although rc.2 behaved as designed: check
connection (`GetVersion` only, no 207) about 4 s after OBS opened, real
session 10 s later, answered at once while OBS's window was already
frozen. OBS's wait chain: its own threads waiting on each other, no other
process. Then two series of 3 starts, a watcher listing every TCP client
of port 4455 every 250 ms:

| Series | Padboard | Clients of 4455 | Frozen |
|---|---|---|---|
| A | Quit | None | **3 / 3** (`AppHangB1`) |
| B | Running, *Piloter OBS* off | None | **3 / 3** (`AppHangB1`) |

Every start froze the same way, 8–9 s after OBS opened: OBS starts GoPro
Webcam (≈ 5 s), the StreamElements plugin removes the scene *Scene* and
reloads its sources, the media source *Trailer Ligue F1 1* reads its
settings twice (*UDTA parsing failed*), and that is the last line of
OBS's UI thread; *ne répond pas* about 5 s later, still frozen after
2 minutes. Plugins besides OBS's own: StreamElements (obs-streamelements-core
26.9.4), Stream Deck (5.5.4.2), Logitech (`logi_obs_plugin_x64`); OBS
33.0.1 runs as administrator. So this freeze is OBS's (or a plugin's, in
this scene collection), not Padboard's: **it does not block 0.2.1**. The
rc.2 changes stay (check before connecting, 10 s wait, one
`GetInputMute` batch, `padboard.log.1`): they ask nothing during the
load. The 5 starts with a tablet button could not be run (OBS frozen
every time).

To do (Linux session): the check connection is cut instead of closed
(OBS logs close code 1006 *End of File*): send a WebSocket close and
wait for OBS's before dropping it.

### 11.4 First installation on an empty folder

| Check | Do | Expect |
|---|---|---|
| Removed | *Settings → Apps* → Padboard → *Désinstaller*, tick **Supprimer les données de l'application** | Padboard, its folder and its data gone |
| Folder page | Run the 0.2.1-rc.2 installer | No *Déjà installé* page; the **folder page shows**, on `%LOCALAPPDATA%\Padboard`; install, Padboard starts as new (demo profile) |
| Restored | Quit Padboard, copy the saved folder back to `%APPDATA%\io.github.hugoambiehl.padboard`, start Padboard | Your profiles and pairings are back |

**Result (2026-11-19, rc.2)**: passed. The uninstall with the data
removed the program folder, both data folders (`%APPDATA%` and
`%LOCALAPPDATA%\io.github.hugoambiehl.padboard`), the `Run` value, the
install and uninstall keys and the shortcuts; only Windows's own
`StartupApproved\Run` value stayed (harmless without `Run`). The folder
page showed `%LOCALAPPDATA%\Padboard`, changed to `D:\Programme
Files\Padboard`; Padboard started as new (*Démo*, autostart and test
versions off). Both data folders restored (hashes checked): profiles,
pairings and settings back, the `Run` value written again at launch
(logged as *removed by another program*: here, our uninstaller), the
tablet reconnected over USB once its app ran (11.5).

### 11.5 Tablet

8.3 to **0.2.1-rc.2** (versionCode 20102), the tablet in kiosk mode:
updated from the PC, kiosk mode back by itself.

**Result (2026-11-19)**: passed, without touching the tablet. Kiosk mode
turned on from the USB card (no secure lock screen, no account: the lock
screen went away by itself, tablet connected 2 s later), then the USB
update 20099 → 20102 (APK checked, `adb install -r`), kiosk mode back by
itself, connected again 2 s later (`adb exec-out screencap`). The tray
(tooltip, menu, *Ouvrir Padboard*, closing hides, second launch shows
the running window) passed too.

### 11.6 Afterwards

Restore the autostart state noted in 11.0, the sound, then send the
report with the `padboard.log` files.

## 12. Release candidate 0.2.2-rc.1: "Installer adb" again (Bitdefender active)

Same PC and rules as section 11, 0.2.1 installed. On 2026-11-19 Google
republished the Platform-Tools 37.0.3 archive under the same version:
0.2.1 refuses every download (*SHA-256 mismatch*, reported by a user).
0.2.2 pins the new archive (8 188 601 bytes, SHA-256 `89d10836…4f09d`)
and explains a refused download in the dialog and in `padboard.log`.

### 12.0 Before starting

As 11.0 (no folder deleted here). In 0.2.1: *Versions de test* **on**.
No other adb in `PATH` (the USB card must offer *Installer adb…*): if
Padboard's copy is installed, *Supprimer adb* first.

### 12.1 0.2.1 → 0.2.2-rc.1

| Check | Do | Expect |
|---|---|---|
| Update | *Rechercher*, *Télécharger*, *Quitter Padboard*, run the file | As 11.1: no folder page, configuration and `Run` value kept, one entry in *Settings → Apps* |
| Bitdefender | Whole section | No detection, no quarantine |

**Result (2026-11-26)**: passed. Before: both data folders saved
(`%APPDATA%` and `%LOCALAPPDATA%\io.github.hugoambiehl.padboard`, 304
files, every SHA-256 checked, Padboard quit: WebView2 locks four cache
files while it runs). *Rechercher*: *update available
version=0.2.2-rc.1*; *Télécharger*: *installer downloaded and checked*,
*not started by Padboard* (`no trusted publisher in this build`), SHA-256
`e3fb89c7…b4779ab3d5b` = `SHA256SUMS`. *Quitter Padboard* in the card,
double-click: nothing unusual (no folder page, no data checkbox).
Still in `D:\Programme Files\Padboard`, no `%LOCALAPPDATA%\Padboard`;
`identity`, `obs`, `paired-devices`, `preferences`, `profiles`, `tls-key`
unchanged byte for byte, `update-pending.json` removed; one entry in
*Settings → Apps* (0.2.2-rc.1); `Run` value and `StartupApproved` (`02…`)
unchanged (*entry refreshed*); Start menu and desktop shortcuts kept;
*update installed version=0.2.2-rc.1*, tablet reconnected over USB 2 s
after launch. Bitdefender: no change in quarantine, ATC feedback or
firewall rules for the whole section (12.1 to 12.3).

### 12.2 Installer adb

| Check | Do | Expect |
|---|---|---|
| Install | *Installer adb…*, tick, *Télécharger et installer* | Within seconds the card shows *Platform-Tools 37.0.3 de Google, téléchargés par Padboard.*; the tablet connects over USB |
| Signatures | `Get-AuthenticodeSignature "$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\*.exe","$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\*.dll"` | `Valid`, signer `CN=Google LLC` for every file |
| Log | `padboard.log` | *the user accepted the Android SDK license*, then *platform-tools installed*; no *not the pinned archive* |
| Again | *Supprimer adb*, then *Installer adb…* again | Removed then installed again without error |

The refused cases (web page, cut transfer, archive changed by Google)
are covered by unit tests; their texts can be seen in the mock
(`?mock&view=connections&adb=missing&tools=changed`, or `html`,
`truncated`, `unexpected`).

**Result (2026-11-26)**: passed. Two adb were in `PATH` (Touch Portal's
and a WinGet `Google.PlatformTools`, both in the user `PATH`, Touch
Portal's also in the machine one), so Padboard was quit and started from
a PowerShell window whose `PATH` dropped both (`where.exe adb`: nothing);
Windows's `PATH` and Touch Portal untouched, its adb server kept running.
*Install*: *the user accepted the Android SDK license: downloading
platform-tools version="37.0.3"*, *platform-tools installed* 1.6 s later,
then *tracking USB devices* with our copy and *adb reverse active*; no
*not the pinned archive*; the card as expected. *Signatures*: the 8
`.exe` and 2 `.dll` `Valid`, `CN=Google LLC`. Our 37.0.3 client used
Touch Portal's running server (port 5037) without conflict. *Again*:
*adb server started by another program: left running* (Touch Portal's
`adb.exe`), *platform-tools removed*, then installed again without
error, signatures `Valid` again. The refused cases' texts: **verified
under Linux** (unit tests and the mock), not run on this PC. Padboard
then started again from the Start menu (usual `PATH`): it uses Touch
Portal's adb found in `PATH`, as expected (setting, `PATH`, SDK, our
copy).

### 12.3 Tablet

8.3 to **0.2.2-rc.1** (versionCode 20201), the tablet in kiosk mode:
updated from the PC, kiosk mode back by itself.

**Result (2026-11-26)**: passed. Tablet on 0.2.1 (20199) in kiosk mode;
*Mettre à jour vers 0.2.2-rc.1*: *tablet APK downloaded and checked*,
*tablet updated over USB from=20199 to=20201* in 5 s, reconnected 2 s
later. Still device owner, lock task `LOCKED`, Padboard in front, no
kiosk exit in `logcat -s Kiosk`; `<config>/updates` keeps only the
0.2.2-rc.1 APK and `SHA256SUMS` (+ `.sig`).

### 12.4 Afterwards

Restore the autostart state noted in 12.0, the sound, then send the
report with the `padboard.log` files.

**Result (2026-11-26)**: autostart unchanged (`Run` value,
`StartupApproved` `02…`), no step touched it. Sound: playback Logitech
PRO X Gaming Headset 100 %, recording Shure MV6 100 %, neither muted,
noted by hand (no audio action run, nothing to restore). The backup of
12.1 is kept until the tester deletes it.

## 13. Release candidate 0.3.0-rc.1: update from 0.2.2, OBS, Android 7 (Bitdefender active)

Same PC and rules as section 12, 0.2.2 installed. 0.3 changes little on
the PC:

- OBS's check connection is closed cleanly (#15);
- the autostart log line is reworded (#16);
- the PC reads a tablet's device owner on Android 7.0, which has no `dpm list-owners`.

On the tablet, the app runs from Android 7.0 (minSdk 24, #21): new
launcher icons for Android 7, and the screensaver's clock rewritten
without `java.time`. It must look exactly as before.

### 13.0 Before starting

- As 12.0. In 0.2.2: *Versions de test* **on**.
- Note the autostart state (`Run` value, *Démarrage* card) and the sound
  (playback and recording devices, volumes, mute), to restore them in 13.6.
- OBS 33 with obs-websocket on, set up in Padboard (*Connexions* → OBS),
  **closed** for now.

**Result (2026-11-26)**: the PC and the tablet were still on
**0.2.2-rc.1** (tablet versionCode 20201), not 0.2.2: the update was
made from there (same installer and update path as 0.2.2). *Versions de
test* on; autostart on (`Run` value `"…\padboard-desktop.exe" --hidden`,
`StartupApproved` `02…`); sound: playback Logitech PRO X Gaming Headset
100 %, recording Shure MV6 100 %, neither muted (read by hand). OBS set
up (`127.0.0.1:4455`, on) and closed. Padboard quit, both data folders
saved to `D:\padboard-backup-0.2.2` (320 files, every SHA-256 checked
against the copy).

### 13.1 0.2.2 → 0.3.0-rc.1

| Check | Do | Expect |
|---|---|---|
| Update | *Rechercher*, *Télécharger*, *Quitter Padboard*, run the file | As 12.1: no folder page, configuration and `Run` value kept, one entry in *Settings → Apps* (0.3.0-rc.1) |
| Bitdefender | Whole section | No detection, no quarantine |

**Result (2026-11-26)**: passed, from 0.2.2-rc.1. *Rechercher*: *update
available version=0.3.0-rc.1*; *Télécharger*: *installer downloaded and
checked*, *not started by Padboard* (`no trusted publisher in this
build`), SHA-256 `a6d03fb5…388293` as published. *Quitter Padboard* in
the card, double-click: no SmartScreen warning, no folder page, no data
checkbox. Still in `D:\Programme Files\Padboard`, no
`%LOCALAPPDATA%\Padboard`; `identity`, `obs`, `paired-devices`,
`preferences`, `profiles`, `tls-key` unchanged byte for byte,
`update-pending.json` removed; one entry in *Settings → Apps*
(0.3.0-rc.1); `Run` value and `StartupApproved` (`02…`) unchanged
(*entry refreshed*); Start menu and desktop shortcuts kept; *update
installed version=0.3.0-rc.1*, tablet reconnected over USB under 1 s
after launch. Bitdefender: no change in quarantine, ATC feedback or
firewall rules for the whole section (13.1 to 13.4).

### 13.2 OBS closes cleanly (#15)

obs-websocket logs every client that leaves:

```
[obs-websocket] [WebSocketServer::onClose] WebSocket client `…` has disconnected with code `…` and reason: …
```

Up to 0.2.2, every check connection logged code `1006` (*End of File*).

| Check | Do | Expect |
|---|---|---|
| Check connection | Padboard running, start OBS, wait until *Connexions* says OBS is connected, then open OBS's log (*Aide → Fichiers journaux → Afficher le fichier journal actuel*) | The check connection (the first client, about 10 s before the real session) has disconnected with code **`1000`**, never `1006` |
| Settings changed | In *Connexions* → OBS, save the settings again (or change the password and put it back) | Padboard disconnects and connects again; in OBS's log, code **`1000`** for the session that ended |
| Still works | One OBS button from the tablet | It acts as before; OBS never freezes (11.3) |

**Result (2026-11-26)**: passed (OBS 33, obs-websocket 5.7.6). Padboard
running, OBS started: the check connection opened at 18:02:15.738 and
*has disconnected with code `1000`* at 18:02:15.756; the real session
connected 10 s later (18:02:25.768), every initial request answered (8
scenes, 33 inputs). Settings saved again: the session *has disconnected
with code `1000`* (18:04:25.493), the new one connected 3 ms later and
got its answers. A scene button from the tablet worked; OBS never froze.

### 13.3 Autostart log (#16)

| Check | Do | Expect |
|---|---|---|
| Written again | *Démarrage* on; quit Padboard; `Remove-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name Padboard`; start Padboard | `padboard.log`: *the entry was missing (removed outside Padboard: uninstaller, security software, Task Manager…), written again*; the `Run` value is back; `StartupApproved` unchanged |

**Result (2026-11-26)**: passed. `Run` value removed with Padboard quit,
Padboard started from the Start menu: `WARN … autostart: start with the
session: the entry was missing (removed outside Padboard: uninstaller,
security software, Task Manager…), written again`; the `Run` value back,
identical; `StartupApproved` still `02…`.

### 13.4 SM-T580 (Android 12L): no regression

The tablet is in kiosk mode, on 0.2.2 (versionCode 20299).

| Check | Do | Expect |
|---|---|---|
| Update | *Mettre à jour vers 0.3.0-rc.1* in the USB card | *tablet updated over USB from=20299 to=30001*; reconnected; still device owner, lock task `LOCKED`, Padboard in front |
| Kiosk seen by the PC | USB card | Kiosk mode shown **on**, as before (`dpm list-owners` path, unchanged on 12L) |
| Screensaver, French | *Écran de veille* card: delay 30 s (the card's shortest); tablet in French; wait | Black screen, clock `HH:mm` (24 h), date like *Jeudi 26 novembre* (capital first letter, no year), the states row as before; it drifts after a minute; a tap wakes it and presses nothing |
| Screensaver, English | Kiosk mode blocks Android's settings: *Libérer la tablette* in the USB card first; tablet in English (*Settings → System → Languages & input → Languages*, English first), open Padboard, wait again | Date like *Thursday 26 November*, clock unchanged |
| Afterwards | Tablet back to French, *Activer le mode kiosque* in the USB card, screensaver delay back to its previous value | Kiosk mode on again |
| Pairing | The tablet keeps its Wi-Fi pairing (unplug USB: it switches to Wi-Fi without a new PIN) | Pairings stored by 0.2.2 still valid (same base64 encoding) |

**Result (2026-11-26)**: passed, except *Pairing* (not applicable).
*Update*: tablet on 0.2.2-rc.1 (20201), kiosk mode; *tablet APK
downloaded and checked*, *tablet updated over USB from=20201 to=30001*
in 3 s, reconnected 1 s later; `versionName=0.3.0-rc.1`, `minSdk=24`;
still device owner, `LOCKED`, Padboard in front. *Kiosk seen by the PC*:
shown on. Grid identical before and after (screenshots). Pages and
folders: the only profile has one page and no folder, so the tester
added test content in the editor, tried pages and folders on the
tablet (no problem), then deleted it: `profiles.json` back identical
byte for byte. *Screensaver,
French*: the card offers 30 s at the shortest (the backend accepts 15 s,
the plan said 15 s): *Jeudi 26 novembre*, `18:10`, nothing on the states
row (nothing muted on Windows, no recording); the clock moved at the
next minute; a tap (adb, on the active scene's tile) woke it, the grid
came back. *Screensaver, English*: kiosk mode left with *Libérer la
tablette* (*the tablet left kiosk mode by=Pc*), `en-GB` first:
*Thursday 26 November*, clock unchanged. *Afterwards*: `fr-FR` only
again, *Activer le mode kiosque* (*kiosk mode enabled*: device owner,
`LOCKED`), delay back to 300 s. *Pairing*: not applicable, the tablet
was never paired over Wi-Fi (`paired-devices.json` empty, PC Wi-Fi off);
Wi-Fi left off.

### 13.5 Android 7 tablet (the tester's own)

The tablet that refused the 0.2 APK ("problème lors de l'analyse du
package"). Note its model and Android version (*Paramètres → À propos*).

| Check | Do | Expect |
|---|---|---|
| Install | `padboard-0.3.0-rc.1.apk` from the release, opened on the tablet (*Sources inconnues* on, Android 7) or `adb install` | Installed; Padboard's icon in the launcher (square, or round on 7.1) |
| USB | USB debugging on, the tablet plugged in | It connects; the grid shows; a button acts on the PC |
| Wi-Fi | *Appairer un appareil* on the PC, the PIN on the tablet | Paired (same short fingerprint on both); unplugged, it stays connected over Wi-Fi |
| Screensaver | Delay 30 s, wait | Clock and date as on the SM-T580 |
| Kiosk (only if the tablet has **no account**) | *Activer le mode kiosque* in the USB card | Kiosk mode on, shown on in the card; leave it with the corner gesture (5 s, then confirm). On Android 7 with 0.3.x the system launcher then comes in front (#24, expected, fixed in 0.4); Padboard still opens from it |

If the tablet cannot run kiosk mode (accounts on it, or a manufacturer
that refuses `set-device-owner`), note Android's message: everything else
is still worth testing.

**Status (2026-11-26)**: postponed. 0.3.0 is released without it: Android
7.0 is checked on the API 24 emulator only (install, USB, pinned TLS,
kiosk mode, silent update, screensaver), never on a real Android 7
tablet yet. The tester's tablet will be tested with 0.3.0
(`padboard-0.3.0.apk`); #21 stays open until then.

### 13.6 Afterwards

Restore the autostart state and the sound noted in 13.0, then send the
report with the `padboard.log` files, OBS's log of 13.2 and the Android 7
tablet's model and version.

**Result (2026-11-26, without 13.5)**: autostart as in 13.0 (`Run`
value, `StartupApproved` `02…`); sound as in 13.0, checked by the tester
(no audio action run); configuration identical to the backup of 13.0
byte for byte. The backup `D:\padboard-backup-0.2.2` is kept until the
tester deletes it. 13.5 is run separately.

## 14. Release candidate 0.4.0-rc.1: update from 0.3.0, "Installer adb", kiosk exit (Bitdefender active)

Same PC and rules as section 13, 0.3.0 installed, the SM-T580 on 0.3.0
(versionCode 30099) in kiosk mode. 0.4 changes:

- "Installer adb" accepts an archive Google republished under the same
  version, when Google's index confirms it (#18). That case cannot be
  produced on demand: here only the usual path is checked, which must not
  change;
- on the tablet, leaving kiosk mode no longer lets the system launcher
  come in front on Android 7 (#24): the app now ends its own lock task
  first. On the SM-T580 (Android 12L) nothing must change.

### 14.0 Before starting

- As 13.0. In 0.3.0: *Versions de test* **on**.
- Note the autostart state (`Run` value, `StartupApproved`, *Démarrage*
  card) and the sound (playback and recording devices, volumes, mute), to
  restore them in 14.4.
- Quit Padboard and save both data folders (as in 13.0, e.g. to
  `D:\padboard-backup-0.3.0`).

### 14.1 0.3.0 → 0.4.0-rc.1

| Check | Do | Expect |
|---|---|---|
| Update | *Rechercher*, *Télécharger*, *Quitter Padboard*, run the file | As 13.1: *update available version=0.4.0-rc.1*, *installer downloaded and checked*, *not started by Padboard*; SHA-256 as published; no folder page, configuration and `Run` value kept, one entry in *Settings → Apps* (0.4.0-rc.1) |
| Bitdefender | Whole section | No detection, no quarantine, no ATC feedback |

### 14.2 "Installer adb": the usual path (#18)

This PC has two other adb in `PATH` (Touch Portal's and WinGet's
`Google.PlatformTools`): as in 12.2, Padboard is started from a
PowerShell window whose `PATH` leaves them out, so the offer shows.
Windows's `PATH` and Touch Portal are not changed; Touch Portal's adb
server may keep running.

1. Quit Padboard (tray icon → *Quitter*).
2. In PowerShell:

   ```powershell
   $env:PATH = ($env:PATH -split ';' | Where-Object { $_ -and -not (Test-Path (Join-Path $_ 'adb.exe')) }) -join ';'
   where.exe adb   # must print nothing
   & 'D:\Programme Files\Padboard\padboard-desktop.exe'
   ```

| Check | Do | Expect |
|---|---|---|
| Install | If Padboard's own copy is there, *Supprimer adb* first; then *Installer adb…*, tick, *Télécharger et installer* | The card shows `…\platform-tools\adb.exe` and *Platform-Tools 37.0.3 de Google, téléchargés par Padboard.*, **no orange line** under it; the tablet connects over USB |
| Log | `padboard.log` | *the user accepted the Android SDK license*, then *platform-tools installed*; neither *not the pinned archive* nor *accepted from Google's index* |
| Folder | `Test-Path "$env:LOCALAPPDATA\io.github.hugoambiehl.padboard\platform-tools\padboard-from-index.json"` | `False` |
| Signatures | As 12.2 (`Get-AuthenticodeSignature … *.exe, *.dll`) | `Valid`, `CN=Google LLC` for every file |
| Back to normal | Quit Padboard, start it from the Start menu | It uses the adb found in `PATH` again (Touch Portal's), as at the end of 12.2 |

The texts of the fallback (orange line, log at WARN) can be seen in the
mock: `?mock&view=connections&adb=missing&tools=index`, then *Installer
adb…*.

### 14.3 SM-T580 (Android 12L): update and kiosk exit (#24)

| Check | Do | Expect |
|---|---|---|
| Update | *Mettre à jour vers 0.4.0-rc.1* in the USB card | *tablet updated over USB from=30099 to=40001*; reconnected; still device owner, lock task `LOCKED`, Padboard in front |
| Exit by the gesture | Hold the bottom-left corner 5 s, confirm *Quitter le mode kiosque* | **Padboard stays in front** (not the launcher); status bar and navigation back; `adb shell dpm list-owners`: no owner; the PC's USB card shows kiosk mode off |
| Kiosk on again | *Activer le mode kiosque* in the USB card | Device owner, `LOCKED`, Padboard in front |
| Exit from the PC | *Libérer la tablette* in *Tablettes connectées* (two clicks) | Same as *Exit by the gesture*; `padboard.log`: *the tablet left kiosk mode* with `by=Pc` |
| Kiosk on again | *Activer le mode kiosque* | As above; leave the tablet in kiosk mode |

### 14.4 Afterwards

Restore the autostart state and the sound noted in 14.0, compare the
configuration with the backup of 14.0 (only `update-pending.json` may
differ), then send the report with the `padboard.log` files.

## 15. Release candidate 0.4.0-rc.2: one screen in kiosk mode, through sleep

0.4.0-rc.1 was never published: rc.2 replaces it. Run section 14 with
0.4.0-rc.2 instead (tablet versionCode 40002), then 15.1 before 14.4.

rc.1 was stopped by a loop seen on the SM-T580: after the PC turned kiosk
mode on, the first wake-up of the screen started the app a second time
(the home entry, `HomeAlias`, beside the activity the PC had started), and
the two connections of the same tablet replaced each other on the PC
every 0.52 s (about 4,000 `session superseded` an hour; the PC's window
seemed to reload every second). rc.2: kiosk mode runs in the home entry
only, one connection per app, a tablet replaced by its own other session
waits longer and longer before taking the PC back, and the PC logs such a
loop.

### 15.1 SM-T580: kiosk mode, sleep, wake-up

The checks of [tablet-test-plan.md](tablet-test-plan.md), section
*Release candidate 0.4.0-rc.2*, with this PC.

### 15.2 Results (reported 2026-11-27)

Section 14 run with 0.4.0-rc.2, then 15.1. The tablet's power button
and touches were simulated with `adb shell input` (tester away), the
PC's window driven through UI Automation.

- **14.0**: 0.3.0 installed, *Versions de test* on; autostart on (`Run`
  value `"…\padboard-desktop.exe" --hidden`, `StartupApproved` `02…`);
  sound given by the tester: playback *Haut-parleurs (Logitech PRO X
  Gaming Headset)* 100 %, recording *Microphone (2- Shure MV6)* 100 %,
  neither muted. The SM-T580 on 0.3.0 (30099) in kiosk mode, still in
  the rc.1 loop (`.MainActivity` t58 + `.HomeAlias` t59). Padboard quit,
  both data folders saved to `D:\padboard-backup-0.3.0` (320 files,
  every SHA-256 checked, `SHA256-manifest.txt` inside).
- **14.1**: passed. *update available version=0.4.0-rc.2* (already
  found by the launch check, again by *Rechercher*); *Télécharger*:
  *installer downloaded and checked*, *not started by Padboard* (`no
  trusted publisher in this build`), SHA-256 `8cb39124…352428` as
  published, no `Zone.Identifier`. *Quitter Padboard*: *quit from the
  window*, *exited*. Installer: no SmartScreen warning, *Déjà installé*
  page (uninstall first, default), no folder page, no data checkbox.
  Still in `D:\Programme Files\Padboard`, no `%LOCALAPPDATA%\Padboard`;
  `identity`, `obs`, `paired-devices`, `preferences`, `profiles`,
  `tls-key` unchanged byte for byte, `update-pending.json` removed at
  launch; one entry in *Settings → Apps* (0.4.0-rc.2); `Run` value and
  `StartupApproved` unchanged (*entry refreshed*); shortcuts kept;
  *update installed version=0.4.0-rc.2*. While the 0.3.0 tablet still
  looped, the new PC logged *this tablet keeps replacing its own session
  (5 in a minute): two copies of the app's connection on the tablet?*
  2 s after launch, as intended. Test-driving note: a keystroke meant
  for the installer reached the Explorer window Padboard had opened on
  the file and started the installer a second time; that second copy
  installed, the first one (still on its welcome page) was closed.
  Bitdefender: no change in quarantine, ATC feedback or firewall rules
  for the whole section.
- **14.2**: passed, after the tablet update. Padboard started with a
  `PATH` without adb (`where.exe adb`: nothing); our copy from 12.2 was
  there, so *Supprimer adb* first: **refused twice with *Accès refusé.
  (os error 5)*** while the 0.3.0 tablet looped: each of its new
  sessions (every 0.5 s) made the PC run `dpm list-owners` with our
  `adb.exe`, so the file was nearly always running (nothing was
  deleted). Once the tablet was on 40002 (no loop): *adb server
  started by another program: left running* (Touch Portal's),
  *platform-tools removed*. *Installer adb…*: *the user accepted the
  Android SDK license*, *platform-tools installed* 1.1 s later,
  *tracking USB devices* with our copy, *adb reverse active*; the card
  shows the path and *Platform-Tools 37.0.3 de Google, téléchargés par
  Padboard.*, no orange line; neither *not the pinned archive* nor
  *accepted from Google's index*; no `padboard-from-index.json`; the 8
  `.exe` and 2 `.dll` `Valid`, `CN=Google LLC`, identical to the 12.2
  copy. *Back to normal*: started from the Start menu, Touch Portal's
  adb from `PATH` again.
- **14.3 and 15.1**: passed, see the results in
  [tablet-test-plan.md](tablet-test-plan.md) (*Release candidate
  0.4.0-rc.2*). *Mettre à jour vers 0.4.0-rc.2*: *tablet updated over
  USB from=30099 to=40002* in 3.5 s; `KioskUpdated` brought back one
  screen only (`.HomeAlias`), `LOCKED`, device owner; the loop ended
  with the 0.3.0 process. From then on: one session (`client=1`) from
  10:55:10Z to the end of the tests, through two sleeps, two kiosk
  exits and two activations; **0 `session superseded`, 0 *keeps
  replacing***, no WARN or ERROR in `padboard.log`. Test-driving note:
  the second click of *Libérer la tablette* sent through UI Automation's
  Invoke never confirmed (the button lost focus between the two
  Invokes, and `@blur` disarms it); two Enter key presses on the
  focused button 1.5 s apart did, as a user's two clicks.
- **14.4**: autostart as in 14.0 (`Run` value, `StartupApproved`
  `02…`); configuration identical to the backup except the logs and
  `updates/` (the 0.4.0-rc.2 APK and its `SHA256SUMS` in place of
  0.3.0's); sound as in 14.0, checked by the tester (no audio action
  run). The
  backup `D:\padboard-backup-0.3.0` is kept until the tester deletes it.
  The tablet is left in kiosk mode.

## 16. Release candidate 0.4.0-rc.3: local network on Android 17, kiosk exit

Same PC and rules as section 13, 0.4.0-rc.2 installed, the SM-T580 on
0.4.0-rc.2 (versionCode 40002) in kiosk mode. rc.3 changes only the
tablet app (#31, #32); the PC side is the same as rc.2.

| Check | Do | Expect |
|---|---|---|
| Update | As 14.1, 0.4.0-rc.2 → 0.4.0-rc.3 | As 14.1 (SHA-256 as published, configuration and `Run` value kept); Bitdefender: nothing |
| Tablet over USB | *Mettre à jour vers 0.4.0-rc.3* in the USB card | *tablet updated over USB from=40002 to=40003*; reconnected; still device owner, `LOCKED`, Padboard in front |
| Tablet over Wi-Fi | Unplug the cable | Reconnected over Wi-Fi within seconds, no question on the tablet |
| SM-T580 | The checks of [tablet-test-plan.md](tablet-test-plan.md), section *Release candidate 0.4.0-rc.3* | As written there |
| No loop | `padboard.log` after the whole section | Never *keeps replacing its own session* |

Then restore the autostart state and the sound as in 14.4.

### 16.1 Results (reported 2026-11-27)

The tablet's power button and touches were simulated with `adb shell
input` (tester away), the PC's window driven through UI Automation; the
installer was started and run by the tester.

- **Before**: 0.4.0-rc.2 installed, *Versions de test* on, Padboard's
  Wi-Fi off (`lanEnabled: false`); autostart on (`Run` value
  `"…\padboard-desktop.exe" --hidden`, `StartupApproved` `02…`); sound
  given by the tester, as in section 15 (Logitech PRO X 100 %, Shure
  MV6 100 %, neither muted). The SM-T580 on 40002 in kiosk mode, one
  screen `.HomeAlias`, `LOCKED`. Padboard quit, both data folders saved
  to `D:\padboard-backup-rc2` (322 files, every SHA-256 checked,
  `SHA256-manifest.txt` inside).
- **Update**: passed. *update available version=0.4.0-rc.3* (launch
  check, then *Rechercher*); *Télécharger*: *installer downloaded and
  checked*, *not started by Padboard* (`no trusted publisher in this
  build`), SHA-256 `835ceb74…7022c3` as published, no
  `Zone.Identifier`; *Quitter Padboard*: *quit from the window*,
  *exited*. Installer double-clicked by the tester: nothing unusual.
  Same folder, uninstall entry 0.4.0-rc.3, configuration identical to
  the backup except the logs and `updates/` (the 0.4.0-rc.3 APK and its
  `SHA256SUMS` in place of rc.2's); `Run` value and `StartupApproved`
  unchanged (*entry refreshed*); *update installed
  version=0.4.0-rc.3*. Bitdefender: no change in quarantine, ATC
  feedback or firewall rules for the whole section.
- **Tablet over USB**: passed. *tablet updated over USB from=40002
  to=40003* 3 s after the click; reconnected 1 s later; device owner,
  `LOCKED`, one screen `.HomeAlias` in front.
- **Tablet over Wi-Fi**: not done (cable to unplug by hand, Padboard's
  Wi-Fi off on this PC); Wi-Fi is checked on a friend's Android 7
  tablet.
- **SM-T580**: passed except the two Wi-Fi lines (not done), see the
  results in [tablet-test-plan.md](tablet-test-plan.md) (*Release
  candidate 0.4.0-rc.3*).
- **No loop**: 0 `session superseded`, 0 *keeps replacing*, no WARN or
  ERROR in `padboard.log` for the whole section.
- **Afterwards**: autostart as before; sound as before, checked by the
  tester (no audio action run). The backup `D:\padboard-backup-rc2` is kept. The tablet
  is left in kiosk mode.
- Test-driving note: two Enter presses meant for *Libérer la tablette*
  were sent while UI Automation reported the button focused but
  Padboard's window was not in front: they went to whichever window
  was (no installer was started, Padboard's state unchanged). The script
  now brings the window forward by its exact title (`AppActivate
  'Padboard'`), sends the second Enter only once the button reads
  *Confirmer : quitter le mode kiosque*, and the screen-off release was
  done again from the start.

## 17. 0.5: "Supprimer adb" while our adb runs (#35)

0.4.0-rc.2 refused *Supprimer adb* with *Accès refusé. (os error 5)*
while our `adb.exe` was running (15.2). From 0.5 the removal puts adb on
hold, stops our server and waits for it, then moves the folder aside,
retried for about 5 s.

| Step | Do | Expected |
|---|---|---|
| Busy copy | Padboard's own copy installed, the tablet plugged over USB and connected; in a terminal, keep our copy busy: `…\platform-tools\adb.exe -s <serial> shell sleep 3`, then at once **Supprimer adb** | The folder is gone (no `platform-tools.removing` left either); no error; `padboard.log`: *adb on hold*, *our adb server stopped*, possibly *platform-tools in use, trying again*, then *platform-tools removed*, *adb hold released* |
| Still in use | Install again; start `…\platform-tools\adb.exe -s <serial> shell sleep 30`, then **Supprimer adb** | After about 5 s, the clear message (*adb n’a pas pu être supprimé : un de ses fichiers est encore utilisé…*), not *os error 5*; the folder is still there and whole, and the USB connection comes back; once the command is over, **Supprimer adb** works |
| Another program's server | As in 8.2 (Touch Portal's adb running) | Unchanged: the other server keeps running |
| Back | *Installer adb…* again | Installed without error |

## 18. 0.5: only the adb server may connect to the USB port (#20)

From 0.5 the PC checks, for every connection to the USB port (47321),
which process opened it (`GetExtendedTcpTable`): it must be the one
listening on the adb server's port (5037). Anything else is refused,
logged (*USB connection refused*, with the PID and the program) and shown
in the *Appareils USB* card for 5 minutes. A connection that cannot be
checked is refused too, explained on the PC and on the tablet.

| Step | Do | Expected |
|---|---|---|
| Padboard's adb | Only Padboard's own copy (*Installer adb*), the tablet plugged in | Connected over USB as before; `padboard.log` (debug) *USB connection from the adb server*; nothing in the USB card |
| The SDK's adb | Remove Padboard's copy; choose the SDK's `adb.exe` (Android Studio's `platform-tools`), unplug and plug the tablet back in | Connected over USB |
| Touch Portal running | Touch Portal started first, so its adb 28.0.3 runs the server on 5037 (as in 8.2); then Padboard, tablet plugged in | Connected over USB (the server is Touch Portal's adb, still the one listening on 5037): no refusal |
| Another program | In PowerShell: `$c = New-Object Net.Sockets.TcpClient('127.0.0.1', 47321)` | Refused: the USB card says the connection came from `powershell.exe` (its PID), not from the adb server; `padboard.log` has the same; the tablet stays connected. `$c.Close()` |
| Back | Unplug and plug the tablet back in | Connected over USB; the refusal disappears from the card after 5 minutes |
## 19. 0.5: "Lancer une application" in the library

The library's *Documents* entry became three entries using the same open
action: *Lancer une application*, *Ouvrir un fichier ou un dossier*
(Documents by default), *Ouvrir un site web*.

| Step | Do | Expected |
|---|---|---|
| Pick a program | Drag *Lancer une application* onto an empty cell; in the inspector, **Application…** | The dialog opens in the Start menu's `Programs` folder, filtered on programs and shortcuts (`.exe`, `.lnk`) |
| Suggestion | Pick `Discord.lnk` (or any shortcut) | The target is the shortcut; the button is named after it (*Discord*) with a fitting icon; a label or icon chosen before is kept |
| Run it | **Tester l'action**, then from the tablet | The program starts |
| Site | Drag *Ouvrir un site web*, type `https://example.org`, test | The default browser opens the page |

Microsoft Store apps have no `.exe` to pick: #47 (0.6) lists installed apps instead.

## Known Windows limits

- Keys cannot be sent to elevated (administrator) windows.
- Audio states are read every 300 ms (Windows has no simple change feed).
- The installer has no Authenticode signature (SignPath Foundation declined for now): SmartScreen warns on first launch, expected; Padboard never starts it by itself.
