# Padboard on Windows: install and test

How to put the desktop app on a Windows 10/11 PC, connect the tablet over
USB and Wi-Fi, and check every action. The installer is not signed yet.

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
`%APPDATA%\io.github.hugoambiehl.padboard\`.

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
| Open | `https://example.org`, `%USERPROFILE%\Documents`, a `.txt` file, `notepad.exe`, a Start menu `.lnk` | Each opens with its default handler |
| Lock | *Verrouiller* | The session locks (as Win + L) |
| Navigation | Next / previous page, folder, back | As on Linux |
| Multi action | Micro, pause 500 ms, volume +, next page | Steps in order; pressing again while it runs does nothing |
| OBS | OBS Studio 28+: *Tools → WebSocket Server Settings* → enable, copy port and password into *Connexions → OBS* | Status *Connecté*; scene buttons (active scene highlighted), *Enregistrer* (REC badge), *Live* with a real service (EN DIRECT badge), source mute (red); OBS closed: OBS tiles dimmed, back by themselves when it reopens |

Also check: the tablet shows the right states right after connecting, after
a cable unplug/replug, and after the PC wakes from sleep.

## 5. Notification area and startup

| Check | Do | Expect |
|---|---|---|
| Tray icon | Start Padboard | The Padboard icon is in the notification area (maybe under the `^` overflow); its tooltip and menu show the connected tablet (*Galaxy Tab A · USB*) or *Aucune tablette connectée* |
| Tray menu | Left click the icon; right click → *Ouvrir Padboard* / *Quitter Padboard* | Left click and *Ouvrir* show the window; *Quitter* closes Padboard (the tablet loses the PC) |
| First close | Close the window (X) for the first time | A dialog explains Padboard stays in the notification area: *Réduire…* hides the window, the tablet keeps working; *Quitter* quits |
| Next closes | Close the window again | It hides at once, no dialog |
| Second launch | Start Padboard from the Start menu while it runs hidden | The running window comes back (no second Padboard) |
| Startup | *Connexions* → *Démarrage* → turn on, sign out and in | Padboard starts hidden in the notification area and the tablet connects; turned off: it does not start (*Task Manager → Startup apps* lists it while on) |

## Known Windows limits

- Keys cannot be sent to elevated (administrator) windows.
- Audio states are read every 300 ms (Windows has no simple change feed).
- The installer is not signed yet (SmartScreen warning); SignPath once the project is public.
