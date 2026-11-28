# Tablet test plan

Manual checks on the real tablet (Samsung Galaxy Tab A 10.1 SM-T580,
LineageOS 19.1 = Android 12L) for what the emulator cannot show: system
gestures, kiosk mode (device owner), the real screen's edges. The
automated part runs first: `scripts/tablet-ui-tests.sh` (see
[development.md](development.md#tablet-app)).

Commands are run on the PC with the tablet plugged in over USB. Check the
navigation mode first (*Settings → System → Gestures → System navigation*:
gestures or buttons) and note it with the results; if both can be tried,
do the bottom-edge checks with each.

Logs:

- tablet: `adb logcat -d -s Kiosk` (kept only a short while: read it right
  after each check);
- PC: `padboard.log` in Padboard's configuration folder (Linux:
  `~/.config/io.github.hugoambiehl.padboard/`; Windows:
  `%APPDATA%\io.github.hugoambiehl.padboard\`), started over at each
  launch of Padboard.

Device owner: `adb shell dpm list-owners` (Padboard listed = kiosk mode on;
nothing = off).

## 0.5: the second screen (#20)

Checked out of kiosk mode with the *Padboard (dev)* build on 2026-11-28
(test pattern and the real Linux screen, 5 starts and exits in a row,
volume long press simulated with `adb shell input keyevent --longpress`).
To do again on the release candidate, in kiosk mode.

| Step | Do | Expected |
|---|---|---|
| Question on the PC | Press *Second écran* from a tablet never allowed | Padboard's window comes forward: « … veut afficher un second écran de ce PC », *Refuser* selected; *Refuser* → the tablet says it was refused; again, *Autoriser* → it starts. Over USB with the real app (adb check on) the device then appears in *Connexions* → *Second écran autorisé* and is not asked again; *Retirer* → asked again |
| First time on Linux | Press *Second écran* | GNOME asks to share the screen; accepted, the tablet shows a new, empty monitor (wallpaper) with "Maintiens une touche de volume pour revenir aux boutons" for 4 s |
| Next times | Leave, press again | No question from GNOME; the picture comes back in under a second |
| Screensaver | Second screen on, no touch for longer than the screensaver's delay | The picture is never dimmed; after leaving, the screensaver comes after its delay as usual |
| Volume key exit, kiosk mode | Kiosk mode on (lock task); start the second screen; hold a volume key ~1 s | Back to the grid; the system's volume panel does not show, the volume does not change; `adb logcat -d -s Kiosk` shows no lock task warning |
| Short press | During the second screen, a short press on a volume key | Nothing (consumed: no volume change, no panel); the picture stays |
| After leaving | Back on the grid, press a volume key | The volume changes as usual |
| Stop from the PC | *Arrêter* in Padboard's header, then in the tray menu | Back to the grid on the tablet each time |

## Release candidate 0.4.0-rc.3: Android 17 local network, kiosk exit

- **#31**: on Android 17, the app asks for the local network permission
  ("Nearby devices") only when Wi-Fi is wanted, and grants it itself in
  kiosk mode; it no longer opens the system's device picker at launch.
  The SM-T580 (Android 12L) never asks: nothing must change there.
- **#32**: leaving kiosk mode while a screen of the app that is not in
  front would have been closed. With the screen off this did not happen
  (the app stays in front while the screen sleeps): checked below.

PC and tablet both on 0.4.0-rc.3. First run the 0.4.0-rc.2 section
above again (same results), then:

| Check | Do | Expected |
| --- | --- | --- |
| No question on 12L | Uninstall nothing; quit Padboard on the tablet, start it again, pair over Wi-Fi (*Associer en Wi-Fi*) | No permission dialog, no *Autoriser le Wi-Fi* button; PCs listed as before; pairing and Wi-Fi connection as before |
| USB only | Unplug the Wi-Fi of the PC (or turn Padboard's Wi-Fi off in its status screen), plug the cable | Connected over USB as before |
| Released, screen off | Kiosk mode on; short press on the power button (screen off); on the PC *Libérer la tablette* (two clicks); after 10 s press the power button | Padboard in front as a normal app: one screen, `.MainActivity`; the home query of the 0.4.0-rc.2 section does not list Padboard; `dpm list-owners` lists nothing |
| Kiosk on again | *Activer le mode kiosque* | One screen, `.HomeAlias`, `LOCKED`; leave the tablet in kiosk mode |

Android 17 itself is checked on the emulator only
(`LocalNetworkTest`, on demand, see [development.md](development.md#tablet-app)):
no picker at launch, kiosk mode grants the permission. On a phone or
tablet with Android 17, if one is at hand: first launch, *Associer en
Wi-Fi* shows *Autoriser le Wi-Fi*; *Autoriser* lists the PCs; refusing
twice makes the button open Padboard's settings; USB works throughout.

**Result (2026-11-27, Windows PC of section 16 of
[windows-test-plan.md](windows-test-plan.md))**: passed, except the two
Wi-Fi lines, not done. Started from kiosk mode, updated from 40002 over
USB in kiosk mode; power button and touches simulated with `adb shell
input` (`KEYCODE_POWER`, a 5.5 s `swipe` on the spot at (20, 1180), a
`tap` on the red button), tester away.

- *Update*: from=40002 to=40003; `KioskUpdated` started `.HomeAlias`
  only (t65), `LOCKED`, device owner, connected 1 s later. No
  permission dialog, no picker: `.HomeAlias` focused, the grid on
  screen; `ACCESS_LOCAL_NETWORK` is declared but unknown to 12L (no
  grant state); the log shows only the usual *PermissionController:
  Updating user sensitive* of a package update.
- *No question on 12L*, *USB only*: **not done**. Padboard's Wi-Fi is
  off on this PC (`lanEnabled: false`) and 12L has no local network
  permission; Wi-Fi is checked on a friend's Android 7 tablet. USB
  worked throughout.
- *Released, screen off* (#32): screen off 13:42:29 (`Dozing`),
  *Libérer la tablette* at 13:42:45 / 13:42:47 (screen still off): *the
  tablet left kiosk mode* `by=Pc`; screen still off 10 s later, one
  screen `.MainActivity` t67, `NONE`; screen on 13:42:58: Padboard in
  front (`.MainActivity` focused, the grid, connected), `dpm
  list-owners`: no owners, the HOME query lists Launcher3 and
  FallbackHome only, status bar no longer disabled (`mDisabled1=0x0`).
  The tablet reconnected once at the release (PC `client=2` → `3`,
  0.3 s, no `superseded`).
- *Kiosk on again*: one screen `.HomeAlias` t68, `LOCKED`, device owner.
- The 0.4.0-rc.2 section again: *Sleep* 13:43:35 → 13:44:36: one screen
  t68, `LOCKED`; *No loop* (10 min after the wake-up): 0 `session
  superseded`, 0 *keeps replacing*, no WARN or ERROR, same session
  (`client=3`), tablet: no *replaced by another session of this
  tablet*; *Leave by the corner*: confirmation at 5 s, *Quitter le mode
  kiosque*: `.MainActivity` t69, no owners, HOME query without Padboard,
  `by=Gesture`; *Again, with sleep*: `.HomeAlias` t70, sleep 13:55:32 →
  13:56:33: same screen, `LOCKED`; *Leave from the PC* (screen on):
  `by=Pc`, `.MainActivity` t71; *Kiosk on again*: one screen
  `.HomeAlias` t72, `LOCKED`, device owner; left in kiosk mode. The
  session `client=3` lasted from 12:42:47Z to the end (two kiosk exits
  and two activations, one sleep, without reconnecting).

Navigation mode not noted (no bottom-edge check in this section).

## Release candidate 0.4.0-rc.2: one screen in kiosk mode, through sleep

rc.1 was stopped by a loop on this tablet: after the PC turned kiosk mode
on, the first wake-up of the screen started the app a second time (the
home entry, `HomeAlias`, beside the activity the PC had started), and the
two connections of the same tablet replaced each other on the PC every
0.52 s (about 4,000 `session superseded` an hour; the PC's window seemed
to reload every second). In rc.2 kiosk mode runs in the home entry only,
the app has one connection whatever its screens, a tablet replaced by its
own other session waits longer and longer before taking the PC back, and
the PC logs such a loop. The emulators keep this case only on Android 12L
(`KioskSingleInstanceTest`, API 32 image); a real wake-up is checked here.

PC and tablet both on 0.4.0-rc.2, the tablet out of kiosk mode, Padboard
open on it. `adb shell dumpsys activity activities | grep -E "Hist +#.*padboard"`
lists the app's screens ("one screen" below: exactly one line).

| Check | Do | Expected |
| --- | --- | --- |
| Kiosk on | *Activer le mode kiosque* in the PC's USB card | One screen, `.HomeAlias`; `dumpsys activity activities` shows `mLockTaskModeState=LOCKED`; Padboard in front, connected |
| Sleep | Short press on the power button (screen off), wait 1 min, press it again | Padboard in front, connected again within seconds; still one screen, `.HomeAlias`; `LOCKED` |
| No loop | Leave it connected 10 min, then read the PC log | At most a few `session superseded` (USB ↔ Wi-Fi switches), never *keeps replacing its own session*; *Tablettes connectées* keeps the same *connecté depuis* time; tablet log (`adb logcat -d -s ConnectionManager`): no *replaced by another session of this tablet* |
| Leave by the corner | Hold the bottom-left corner 5 s, confirm after 2 s | Padboard stays in front as a normal app: one screen, `.MainActivity`; `adb shell cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.HOME` does not list Padboard; `dpm list-owners` lists nothing |
| Again, with sleep | *Activer le mode kiosque*, then *Sleep* as above | As *Kiosk on* and *Sleep* |
| Leave from the PC | *Libérer la tablette* in *Tablettes connectées* (two clicks) | As *Leave by the corner* |
| Kiosk on again | *Activer le mode kiosque* | As *Kiosk on*; leave the tablet in kiosk mode |

**Result (2026-11-27, Windows PC of section 15 of
[windows-test-plan.md](windows-test-plan.md))**: passed. Started from
kiosk mode (updated from 0.3.0 over USB in kiosk mode, not from kiosk
mode off); power button and touches simulated with `adb shell input`
(`KEYCODE_POWER`, a 5.5 s `swipe` on the spot at (20, 1180), a `tap` on
the red button), tester away.

- *Update*: from=30099 to=40002; `KioskUpdated` started `.HomeAlias`
  only (t60), `LOCKED`, device owner, connected 2 s later.
- *Sleep* (11:57:14 off, `Dozing`; 11:58:17 on): one screen,
  `.HomeAlias` t60, `LOCKED`; the session was never dropped (PC
  `client=1` throughout).
- *No loop*: 10 min after the wake-up, PC: 0 `session superseded`, 0
  *keeps replacing*, *Connectée à 11:55:10* unchanged; tablet: no
  *replaced by another session of this tablet*.
- *Leave by the corner*: the confirmation opened at 5 s; *Quitter le
  mode kiosque*: one screen `.MainActivity` t61, `NONE`, `dpm
  list-owners`: no owners, the HOME query lists Launcher3 and
  FallbackHome only, status bar no longer disabled (`mDisabled1=0x0`,
  the app's full screen still hides it); tablet log *… hidden gesture
  on the tablet*, PC *the tablet left kiosk mode* `by=Gesture`; the USB
  card offers *Activer le mode kiosque*.
- *Again, with sleep*: *Activer le mode kiosque*: `Device owner set`,
  the app started `.HomeAlias` (HOME, uid 10100) and the PC's `am start
  …/.MainActivity` (uid 2000) 1.6 s later added no screen: one screen
  `.HomeAlias` t62, `LOCKED`. Sleep 12:09:53 → 12:10:56: still one
  screen t62, no new `START` at the wake-up, same PC session.
- *Leave from the PC* (screen on): *order to the tablet*
  `order=ExitKiosk sent=true`, *left kiosk mode* `by=Pc`; tablet *… the
  PC (Libérer la tablette)*; as *Leave by the corner* (`.MainActivity`
  t63).
- *Kiosk on again*: one screen `.HomeAlias` t64, `LOCKED`, device
  owner; left in kiosk mode.

Navigation mode not noted (no bottom-edge check in this section).

## Release candidate 0.2.0-rc.2: the kiosk exit corner

The hidden gesture moved from the top-right corner (next to the
connection pill, set off by mistake during a pairing) to the
**bottom-left corner** (40 dp, about 6 mm, right against both edges),
held 5 s without moving. It only exists in kiosk mode. The bottom edge
is where Android 12L's system gestures start: they must not take the
hold over, in kiosk mode or out of it.

PC and tablet both on 0.2.0-rc.2.

### In kiosk mode

Kiosk mode on (*Activer le mode kiosque* in the PC's USB card).

| Check | Do | Expected |
| --- | --- | --- |
| Corner on the grid | Hold the bottom-left corner 5 s, finger still, against both edges | At 5 s, *Quitter le mode kiosque ?* opens. No system bar, taskbar, back arrow or app switcher shows up, before or after |
| Corner held longer | Hold it 10 s, then lift | The confirmation stays open; lifting the finger does not press anything in it |
| Other screens | Same hold: in an open folder; with Padboard quit on the PC (disconnected card, after 1 s); on the screensaver (black screen with the clock) | The confirmation opens on each. On the screensaver the touch only wakes the screen, it never reaches a button |
| Not the corner | Hold 6 s about 1.5 cm above the bottom-left corner, then 1.5 cm to its right | Nothing |
| Old corner | Hold the top-right corner (connection pill, clock) 6 s | Nothing (the pill's menu may open on release: it is the pill's own tap) |
| Swipe from the corner | Swipe up from the bottom-left corner, then from the bottom edge's middle | No system bar and no app switcher (lock task); Padboard stays in front |
| Red button | Open the confirmation, tap the red button at once | Nothing for about 2 s (dimmed button), then it works |
| Cancel | Open it, then each in turn: *Annuler*, Back (if the button bar shows one), a tap outside the dialog, no touch for 30 s | The dialog closes each time; `dpm list-owners` still lists Padboard |
| Leave | Open it, wait 2 s, tap *Quitter le mode kiosque* | The tablet is ordinary again (status bar, launcher); `dpm list-owners` lists nothing; tablet log: *leaving kiosk mode, asked by: hidden gesture on the tablet*; PC log: *the tablet left kiosk mode* `by=Gesture` |
| Double-click on the PC | Double-click *Libérer la tablette* in *Tablettes connectées* | The button only turns red (*Confirmer : quitter le mode kiosque*); the tablet stays in kiosk mode; clicking elsewhere puts the button back |
| From the PC | Kiosk mode on again, then *Libérer la tablette* in *Tablettes connectées* | Kiosk mode left; PC log: *order to the tablet* `order=ExitKiosk`, then *the tablet left kiosk mode* `by=Pc`; tablet log: *… the PC (Libérer la tablette)* |
| Left away from the PC | Kiosk mode on, quit Padboard on the PC, leave kiosk mode with the corner, wait a minute, start Padboard again | Once the tablet is back: PC log *the tablet left kiosk mode* `by=Gesture` with `seconds_ago` ≈ the time it was away, logged once (not again at the next reconnection) |

### Out of kiosk mode

Kiosk mode off, Padboard open on the grid (immersive: system bars hidden).

| Check | Do | Expected |
| --- | --- | --- |
| Corner | Hold the bottom-left corner 6 s, finger still | Nothing: no confirmation (the gesture exists only in kiosk mode), no back, no app switcher, no assistant |
| Corner on the other screens | Same on the setup screen (*Associer en Wi-Fi*) and on the PIN screen | Nothing; no button there is pressed |
| Bottom edge | Swipe up from the bottom edge | The system bars show for a moment and hide again (immersive mode); Padboard keeps working |

Results: navigation mode, then each line OK / not OK with what was seen.
