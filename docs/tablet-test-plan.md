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
