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
