use std::collections::HashSet;

use padboard_actions::{Action, Hotkey, Key, WinKey};
use serde_json::json;

#[test]
fn serialized_name_is_the_w3c_code() {
    for &key in Key::ALL {
        assert_eq!(serde_json::to_value(key).unwrap(), json!(key.code()));
        let parsed: Key = serde_json::from_value(json!(key.code())).unwrap();
        assert_eq!(parsed, key);
        assert_eq!(Key::from_code(key.code()), Some(key));
    }
}

#[test]
fn unknown_code_is_rejected() {
    assert_eq!(Key::from_code("KeyÀ"), None);
    assert!(serde_json::from_value::<Key>(json!("keya")).is_err());
}

#[test]
fn evdev_codes_are_unique_and_non_zero() {
    let mut seen = HashSet::new();
    for &key in Key::ALL {
        let code = key.evdev_code();
        assert_ne!(code, 0, "{key}");
        assert!(seen.insert(code), "{key}: evdev code {code} used twice");
    }
}

#[test]
fn windows_codes_are_unique() {
    let mut seen = HashSet::new();
    for &key in Key::ALL {
        let win = key.win();
        if let WinKey::Scan { code, .. } = win {
            assert!(
                code > 0 && code < 0x80,
                "{key}: scancode {code:#x} out of set-1 range"
            );
        }
        assert!(seen.insert(win), "{key}: {win:?} used twice");
    }
}

/// Spot-checks against `linux/input-event-codes.h` and the Windows scancode table.
#[test]
fn well_known_keys() {
    assert_eq!(Key::KeyA.evdev_code(), 30);
    assert_eq!(Key::ControlLeft.evdev_code(), 29);
    assert_eq!(Key::MetaLeft.evdev_code(), 125);
    assert_eq!(Key::F13.evdev_code(), 183);
    assert_eq!(
        Key::KeyA.win(),
        WinKey::Scan {
            code: 0x1E,
            extended: false
        }
    );
    assert_eq!(
        Key::ArrowUp.win(),
        WinKey::Scan {
            code: 0x48,
            extended: true
        }
    );
    assert_eq!(Key::MediaPlayPause.win(), WinKey::Vk(0xB3));
}

#[test]
fn modifiers() {
    assert!(Key::ControlLeft.is_modifier());
    assert!(Key::MetaRight.is_modifier());
    assert!(!Key::KeyA.is_modifier());
    assert!(!Key::CapsLock.is_modifier());
}

#[test]
fn action_json_shape() {
    let action = Action::Hotkey(Hotkey::new([Key::ControlLeft, Key::AltLeft, Key::KeyT]));
    let value = serde_json::to_value(&action).unwrap();
    assert_eq!(
        value,
        json!({ "type": "hotkey", "keys": ["ControlLeft", "AltLeft", "KeyT"] })
    );
    assert_eq!(serde_json::from_value::<Action>(value).unwrap(), action);
}
