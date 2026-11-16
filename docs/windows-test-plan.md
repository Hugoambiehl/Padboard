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
   use *Installer* in those versions, see section 8.)
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

### 9.2 The new update path, from rc.3 (local test releases)

Section 6 with the installers built from the **v0.2.0-rc.3** tag (versions
of the config files: A = `0.2.0-rc.3`, B = `0.2.0-rc.4`; the variables
accordingly), **Bitdefender active, no exception**. All of section 6's
checks, plus:

| Check | Do | Expect |
|---|---|---|
| Not started by Padboard | *Télécharger* | The Explorer opens on the installer; no installer process starts until you run it; **no Bitdefender event** during the download and while Padboard runs |
| Run by the user | *Quitter Padboard*, double-click the file | Installs like a manual install: no detection, B starts |

### 9.3 USB, adb and tablet

8.2 ("Supprimer adb" next to Touch Portal) and 8.3 (tablet update from
the PC, kiosk on first, to **0.2.0-rc.3**), unchanged.

### 9.4 Afterwards

Reinstall the official rc.3 if section 6 replaced it (*Afterwards* of
section 6); restore the sound; send the report with the `padboard.log`
files.

## Known Windows limits

- Keys cannot be sent to elevated (administrator) windows.
- Audio states are read every 300 ms (Windows has no simple change feed).
- The installer is not signed yet (SmartScreen warning); SignPath once the project is public.
