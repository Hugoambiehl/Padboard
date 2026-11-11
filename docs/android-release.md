# Tablet: release signing and kiosk mode

## Release signing key

The tablet app is signed with one long-lived key. **Losing it, or its
password, means the app can never be updated in place again**: Android only
accepts an update signed with the same key, and in kiosk mode the app (device
owner) cannot even be uninstalled before leaving kiosk mode.

Where things are (never in the repo):

| What | Where |
|---|---|
| Keystore (PKCS12, RSA 4096, alias `padboard`, valid 30 years) | `~/.padboard-signing/padboard-release.p12` |
| Properties read by Gradle (`storeFile`, `keyAlias`, `storePassword`) | `~/.padboard-signing/signing.properties` |
| Pointer to the properties | `padboard.signing=…` in `~/.gradle/gradle.properties` (or `PADBOARD_SIGNING=<path>`) |

Both files are owner-only (`chmod 600`, folder `700`). Without the pointer,
`assembleRelease` builds an unsigned APK.

Certificate SHA-256 (to check a backup or an APK with
`apksigner verify --print-certs`):
`63:7B:B3:C0:7A:1B:C2:A0:C1:0D:92:F4:15:2E:F9:C9:2F:BF:00:C9:D5:10:B3:E7:2F:63:F7:C2:E3:DC:FA:46`

### Back it up (once, and after any change)

1. Copy `padboard-release.p12` to two places that do not share a failure:
   an encrypted USB stick kept offline, and your password manager (as an
   attachment) or an encrypted cloud folder.
2. Store the password (`storePassword` line of `signing.properties`) in your
   password manager, **separately** from the keystore copy.
3. Check a backup: `keytool -list -v -keystore <copy> -storetype PKCS12`
   must show the SHA-256 above.

Never commit the keystore, the properties or the password, never paste the
password in an issue, a log or a CI variable without a reason (the CI does
not sign release builds for now).

### Restore on a new machine

Put the keystore back in `~/.padboard-signing/`, recreate
`signing.properties` from the password manager, add the
`padboard.signing=` line to `~/.gradle/gradle.properties`, then
`./gradlew :app:assembleRelease` and check the certificate.

### Build and install

```sh
cd apps/android
./gradlew :app:assembleRelease
adb install -r app/build/outputs/apk/release/app-release.apk   # update in place, data kept
```

A tablet carrying a build signed with another key (the debug one) needs
`adb uninstall io.github.hugoambiehl.padboard` first: its data (Wi-Fi
pairing) is lost, pair it again.

## Kiosk mode

The app becomes the tablet's **device owner**: it locks itself in front
(lock task: no home, recents, notifications or status bar; the power menu
still works), is the home app (starts at boot), turns the lock screen off and
keeps the screen on (the screensaver dims it).

Requirements: no account on the tablet (*Settings → Accounts*), a single
user, the release build installed.

### Turn it on

With the tablet plugged in over USB: Padboard on the PC → *Connexions* →
*Appareils USB* → **Activer le mode kiosque**. Or by hand:

```sh
adb shell dpm set-device-owner io.github.hugoambiehl.padboard/.kiosk.KioskAdmin
adb shell content insert --uri content://lineagesettings/system --bind name:s:enable_taskbar --bind value:s:0
adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity
```

The second command turns off the Android 12L taskbar of LineageOS, which
otherwise stays drawn over the app in lock task mode (it reports no inset).
The plain navigation bar replaces it outside the app. To get the taskbar
back: same command with `value:s:1`.

### Turn it off (removes the device owner for good)

- From the PC: *Connexions* → *Tablettes connectées* → **Libérer la tablette**
  (click twice).
- On the tablet, even without a PC: hold the **top-right corner** (the clock)
  for 5 seconds, then *Quitter le mode kiosque*.

Both undo every policy, then give up the device owner role
(`clearDeviceOwnerApp`): the tablet is an ordinary one again, Padboard can be
uninstalled. Only the steps above can turn kiosk mode on again.

### Rescue

If the app can no longer start (crash at launch) while it is the device
owner, in this order:

1. **Install a fixed build** signed with the same key
   (`adb install -r …`): updates are allowed in kiosk mode. This is why the
   key backup matters.
2. If adb still works and the app starts at all, use the corner gesture or
   `adb shell am start -n io.github.hugoambiehl.padboard/.MainActivity` then
   the gesture.
3. Last resort, **factory reset from the recovery** (wipes the tablet): power
   off, hold *Volume up + Home + Power* until the recovery shows, *Factory
   reset → Format data / factory reset*, reboot, install Padboard again.
   (`adb shell dpm remove-active-admin` does not work: it is only allowed for
   apps flagged `testOnly`.)
