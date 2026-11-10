# Padboard

DIY Stream Deck: a desktop app (Windows / Linux) that runs actions on your PC,
and an Android app that turns a dedicated tablet into a touch control surface.
Wi-Fi or USB, no cloud, free and open-source tools only.

> 🚧 Early development (phase 1 — foundations). Not usable yet.

## Repository layout

| Path | Content |
|---|---|
| `crates/protocol` | Wire protocol (source of truth, TS/Kotlin types generated with typeshare) |
| `crates/actions` | Action execution (`ActionExecutor`, Linux and Windows implementations) |
| `apps/desktop` | Desktop app (Tauri 2, Vue 3) |
| `apps/android` | Tablet app (Kotlin, Jetpack Compose) |
| `docs/design` | Reference mockups |
| `scripts/gen-types.sh` | Regenerates the TS and Kotlin protocol types |

## Development setup (Linux)

### Linux: uinput access

On Linux, Padboard sends keyboard shortcuts through a **uinput virtual keyboard**.
It works on both X11 and Wayland, for every application, but the desktop app
needs write access to `/dev/uinput`.

Padboard does **not** require adding your user to the `input` group (that would
give read access to every input device, keyboard included). Instead, a udev rule
tags `/dev/uinput` with `uaccess`: systemd-logind then grants access only to the
user of the **active local session**, and revokes it when the session ends or
another user switches in.

Install the rule (once per machine):

```sh
echo 'KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

- The file name must sort **before** `73-seat-late.rules`, which is where
  `uaccess` tags are turned into permissions — hence the `70-` prefix.
- `static_node=uinput` makes `/dev/uinput` exist at boot even before the
  `uinput` kernel module is loaded (opening it loads the module).
- No reboot or logout is needed.

Check the result — your user must appear with `rw-`:

```sh
getfacl -p /dev/uinput
# user:<you>:rw-
```

Then test injection without the app (sends the shortcut after a 3-second countdown):

```sh
cargo run -p padboard-actions --example hotkey -- ControlLeft AltLeft KeyT
```

Keys are W3C [`KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/) names,
i.e. **physical positions**: `KeyQ` is the key labelled "A" on an AZERTY keyboard.

Without the rule, the desktop app keeps running but keyboard shortcuts fail with
an explicit error pointing to this section. To remove the rule:

```sh
sudo rm /etc/udev/rules.d/70-padboard-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --sysname-match=uinput
```

> Security note: with this rule, any program running in your session can create
> a virtual keyboard and inject keystrokes (it cannot read your real keyboard).
> This is the same trust level as an X11 session.
