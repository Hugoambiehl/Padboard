# Release signing

Two keys sign the releases, both kept on the maintainer's PC (never in the
repo, never in CI) and used by `scripts/sign-release.sh`
([releasing.md](releasing.md)):

- the **tablet's key** (Android signature of the APK);
- the **update key** (minisign): signs the Windows installer for the
  desktop's in-app updates and `SHA256SUMS`, which the PC checks before
  sending the APK to a tablet.

## Tablet APK key

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

In kiosk mode the update stops the app; it comes back in front by itself
right after (no need to press home).

A tablet carrying a build signed with another key (the debug one) needs
`adb uninstall io.github.hugoambiehl.padboard` first: its data (Wi-Fi
pairing) is lost, pair it again.

Kiosk mode (turning it on and off, rescue when the app no longer starts):
[install-tablet.md](install-tablet.md#kiosk-mode).

## Update key (desktop updates, signed checksums)

The app embeds the public key (`apps/desktop/src-tauri/updater-key.pub`) and
accepts an update, or an APK for a tablet, only when it is signed by the
private key **for that exact version** (the version is in the signature's
trusted comment, see `scripts/sign-release.sh`).

**Losing it, or its password**, means the installed apps can no longer
update themselves: a new key ships in a version that users install by hand
once. **If it leaks**, anyone able to serve files to the app (a hijacked
GitHub account, for instance) could make it install their program: generate
a new key, publish a version embedding it, and say so in the release notes.

Where things are:

| What | Where |
|---|---|
| Private key (minisign, encrypted with its password) | `~/.padboard-signing/updater.key` |
| Its public key (also in the repo) | `~/.padboard-signing/updater.key.pub` = `apps/desktop/src-tauri/updater-key.pub` |
| The password | your password manager only |

### Create it (once)

A password is mandatory (`sign-release.sh` refuses a key without one).

```sh
cd apps/desktop
npx tauri signer generate -w ~/.padboard-signing/updater.key
chmod 600 ~/.padboard-signing/updater.key ~/.padboard-signing/updater.key.pub
cp ~/.padboard-signing/updater.key.pub src-tauri/updater-key.pub
```

Commit `updater-key.pub` (the release workflow refuses to build without
it). Note the line `minisign public key: …` that
`base64 -d ~/.padboard-signing/updater.key.pub` prints: it identifies the
key and lets you check a backup.

### Back it up (once, and after any change)

1. Copy `updater.key` to the same two places as the APK keystore (the
   encrypted USB stick kept offline, and the password manager as an
   attachment or the encrypted cloud folder).
2. Store its password in the password manager, **separately** from the key
   copies, under its own entry (not the keystore's password).
3. Check a backup: `base64 -d <copy>` starts with
   `untrusted comment: rsign encrypted secret key`, and signing a test file
   with it (`npx tauri signer sign -f <copy> <file>`) asks for the password
   and succeeds. The public key needs no backup: it is in the repo.

### Restore on a new machine

Put `updater.key` back in `~/.padboard-signing/` (`chmod 600`); the public
key is `apps/desktop/src-tauri/updater-key.pub`.
