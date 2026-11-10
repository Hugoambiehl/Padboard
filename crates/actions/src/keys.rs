//! Physical keys, identified by their W3C `KeyboardEvent.code` name.
//!
//! A key is a *position* on the keyboard, not a character: `KeyQ` is the key
//! labelled "A" on an AZERTY keyboard. This is what the editor captures with
//! `KeyboardEvent.code`, and it maps directly to Linux evdev keycodes and
//! Windows set-1 scancodes regardless of the active keyboard layout.
//!
//! References:
//! - <https://www.w3.org/TR/uievents-code/>
//! - `linux/input-event-codes.h`
//! - <https://learn.microsoft.com/windows/win32/inputdev/about-keyboard-input#scan-codes>

use serde::{Deserialize, Serialize};

/// How a key is injected on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WinKey {
    /// Set-1 scancode, sent with `KEYEVENTF_SCANCODE`. `extended` = `0xE0` prefix.
    Scan { code: u16, extended: bool },
    /// Virtual-key code, for keys whose scancode is awkward or unreliable
    /// to inject (Pause, NumLock, media keys).
    Vk(u16),
}

const fn s(code: u16) -> WinKey {
    WinKey::Scan {
        code,
        extended: false,
    }
}

const fn e(code: u16) -> WinKey {
    WinKey::Scan {
        code,
        extended: true,
    }
}

const fn vk(code: u16) -> WinKey {
    WinKey::Vk(code)
}

macro_rules! keys {
    ($( $name:ident => $evdev:literal, $win:expr; )*) => {
        /// A physical key. Serialized as its W3C `KeyboardEvent.code` name
        /// (the variant name), e.g. `"KeyA"`, `"ControlLeft"`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum Key {
            $( $name, )*
        }

        impl Key {
            /// Every supported key.
            pub const ALL: &'static [Key] = &[$( Key::$name, )*];

            /// W3C `KeyboardEvent.code` name.
            #[must_use]
            pub const fn code(self) -> &'static str {
                match self { $( Key::$name => stringify!($name), )* }
            }

            /// Linux evdev keycode (`KEY_*` in `linux/input-event-codes.h`).
            #[must_use]
            pub const fn evdev_code(self) -> u16 {
                match self { $( Key::$name => $evdev, )* }
            }

            /// How to inject the key on Windows.
            #[must_use]
            pub const fn win(self) -> WinKey {
                match self { $( Key::$name => $win, )* }
            }
        }
    };
}

keys! {
    // Writing system keys
    Backquote => 41, s(0x29);
    Digit1 => 2, s(0x02);
    Digit2 => 3, s(0x03);
    Digit3 => 4, s(0x04);
    Digit4 => 5, s(0x05);
    Digit5 => 6, s(0x06);
    Digit6 => 7, s(0x07);
    Digit7 => 8, s(0x08);
    Digit8 => 9, s(0x09);
    Digit9 => 10, s(0x0A);
    Digit0 => 11, s(0x0B);
    Minus => 12, s(0x0C);
    Equal => 13, s(0x0D);
    KeyQ => 16, s(0x10);
    KeyW => 17, s(0x11);
    KeyE => 18, s(0x12);
    KeyR => 19, s(0x13);
    KeyT => 20, s(0x14);
    KeyY => 21, s(0x15);
    KeyU => 22, s(0x16);
    KeyI => 23, s(0x17);
    KeyO => 24, s(0x18);
    KeyP => 25, s(0x19);
    BracketLeft => 26, s(0x1A);
    BracketRight => 27, s(0x1B);
    KeyA => 30, s(0x1E);
    KeyS => 31, s(0x1F);
    KeyD => 32, s(0x20);
    KeyF => 33, s(0x21);
    KeyG => 34, s(0x22);
    KeyH => 35, s(0x23);
    KeyJ => 36, s(0x24);
    KeyK => 37, s(0x25);
    KeyL => 38, s(0x26);
    Semicolon => 39, s(0x27);
    Quote => 40, s(0x28);
    Backslash => 43, s(0x2B);
    IntlBackslash => 86, s(0x56);
    KeyZ => 44, s(0x2C);
    KeyX => 45, s(0x2D);
    KeyC => 46, s(0x2E);
    KeyV => 47, s(0x2F);
    KeyB => 48, s(0x30);
    KeyN => 49, s(0x31);
    KeyM => 50, s(0x32);
    Comma => 51, s(0x33);
    Period => 52, s(0x34);
    Slash => 53, s(0x35);
    IntlRo => 89, s(0x73);
    IntlYen => 124, s(0x7D);

    // Functional keys
    Escape => 1, s(0x01);
    Backspace => 14, s(0x0E);
    Tab => 15, s(0x0F);
    Enter => 28, s(0x1C);
    CapsLock => 58, s(0x3A);
    Space => 57, s(0x39);
    ShiftLeft => 42, s(0x2A);
    ShiftRight => 54, s(0x36);
    ControlLeft => 29, s(0x1D);
    ControlRight => 97, e(0x1D);
    AltLeft => 56, s(0x38);
    AltRight => 100, e(0x38);
    MetaLeft => 125, e(0x5B);
    MetaRight => 126, e(0x5C);
    ContextMenu => 127, e(0x5D);

    // Control pad & arrows
    PrintScreen => 99, e(0x37);
    ScrollLock => 70, s(0x46);
    Pause => 119, vk(0x13);
    Insert => 110, e(0x52);
    Delete => 111, e(0x53);
    Home => 102, e(0x47);
    End => 107, e(0x4F);
    PageUp => 104, e(0x49);
    PageDown => 109, e(0x51);
    ArrowUp => 103, e(0x48);
    ArrowDown => 108, e(0x50);
    ArrowLeft => 105, e(0x4B);
    ArrowRight => 106, e(0x4D);

    // Numpad
    NumLock => 69, vk(0x90);
    Numpad0 => 82, s(0x52);
    Numpad1 => 79, s(0x4F);
    Numpad2 => 80, s(0x50);
    Numpad3 => 81, s(0x51);
    Numpad4 => 75, s(0x4B);
    Numpad5 => 76, s(0x4C);
    Numpad6 => 77, s(0x4D);
    Numpad7 => 71, s(0x47);
    Numpad8 => 72, s(0x48);
    Numpad9 => 73, s(0x49);
    NumpadAdd => 78, s(0x4E);
    NumpadSubtract => 74, s(0x4A);
    NumpadMultiply => 55, s(0x37);
    NumpadDivide => 98, e(0x35);
    NumpadDecimal => 83, s(0x53);
    NumpadEnter => 96, e(0x1C);

    // Function keys (F13–F24 are handy for Discord / OBS global shortcuts)
    F1 => 59, s(0x3B);
    F2 => 60, s(0x3C);
    F3 => 61, s(0x3D);
    F4 => 62, s(0x3E);
    F5 => 63, s(0x3F);
    F6 => 64, s(0x40);
    F7 => 65, s(0x41);
    F8 => 66, s(0x42);
    F9 => 67, s(0x43);
    F10 => 68, s(0x44);
    F11 => 87, s(0x57);
    F12 => 88, s(0x58);
    F13 => 183, s(0x64);
    F14 => 184, s(0x65);
    F15 => 185, s(0x66);
    F16 => 186, s(0x67);
    F17 => 187, s(0x68);
    F18 => 188, s(0x69);
    F19 => 189, s(0x6A);
    F20 => 190, s(0x6B);
    F21 => 191, s(0x6C);
    F22 => 192, s(0x6D);
    F23 => 193, s(0x6E);
    F24 => 194, s(0x76);

    // Media keys
    AudioVolumeMute => 113, vk(0xAD);
    AudioVolumeDown => 114, vk(0xAE);
    AudioVolumeUp => 115, vk(0xAF);
    MediaTrackNext => 163, vk(0xB0);
    MediaTrackPrevious => 165, vk(0xB1);
    MediaStop => 166, vk(0xB2);
    MediaPlayPause => 164, vk(0xB3);
}

impl Key {
    /// Looks a key up by its W3C `KeyboardEvent.code` name.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// Modifier keys (Ctrl, Shift, Alt, Meta/Windows).
    #[must_use]
    pub const fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRight
                | Self::ControlLeft
                | Self::ControlRight
                | Self::AltLeft
                | Self::AltRight
                | Self::MetaLeft
                | Self::MetaRight
        )
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
