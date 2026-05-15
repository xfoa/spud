//! Platform-agnostic key name parsing.
//!
//! Converts logical key names (as used by iced and the wire protocol)
//! into Linux evdev scancodes and wire-protocol mouse button codes.

/// Parse a logical key name into a Linux evdev scancode.
///
/// Supports:
/// - `evdev:N` format (raw scancode)
/// - Common logical names like "Space", "Enter", "Escape", etc.
pub fn parse_key_name(name: &str) -> Option<u16> {
    if let Some(num) = name.strip_prefix("evdev:") {
        return num.parse::<u16>().ok();
    }

    Some(match name {
        "Escape" => 1,
        "Digit1" => 2,
        "Digit2" => 3,
        "Digit3" => 4,
        "Digit4" => 5,
        "Digit5" => 6,
        "Digit6" => 7,
        "Digit7" => 8,
        "Digit8" => 9,
        "Digit9" => 10,
        "Digit0" => 11,
        "Minus" => 12,
        "Equal" => 13,
        "Backspace" => 14,
        "Tab" => 15,
        "KeyQ" => 16,
        "KeyW" => 17,
        "KeyE" => 18,
        "KeyR" => 19,
        "KeyT" => 20,
        "KeyY" => 21,
        "KeyU" => 22,
        "KeyI" => 23,
        "KeyO" => 24,
        "KeyP" => 25,
        "BracketLeft" => 26,
        "BracketRight" => 27,
        "Enter" => 28,
        "ControlLeft" => 29,
        "KeyA" => 30,
        "KeyS" => 31,
        "KeyD" => 32,
        "KeyF" => 33,
        "KeyG" => 34,
        "KeyH" => 35,
        "KeyJ" => 36,
        "KeyK" => 37,
        "KeyL" => 38,
        "Semicolon" => 39,
        "Quote" => 40,
        "Backquote" => 41,
        "ShiftLeft" => 42,
        "Backslash" => 43,
        "KeyZ" => 44,
        "KeyX" => 45,
        "KeyC" => 46,
        "KeyV" => 47,
        "KeyB" => 48,
        "KeyN" => 49,
        "KeyM" => 50,
        "Comma" => 51,
        "Period" => 52,
        "Slash" => 53,
        "ShiftRight" => 54,
        "NumpadMultiply" => 55,
        "AltLeft" => 56,
        "Space" => 57,
        "CapsLock" => 58,
        "F1" => 59,
        "F2" => 60,
        "F3" => 61,
        "F4" => 62,
        "F5" => 63,
        "F6" => 64,
        "F7" => 65,
        "F8" => 66,
        "F9" => 67,
        "F10" => 68,
        "NumLock" => 69,
        "ScrollLock" => 70,
        "Numpad7" => 71,
        "Numpad8" => 72,
        "Numpad9" => 73,
        "NumpadSubtract" => 74,
        "Numpad4" => 75,
        "Numpad5" => 76,
        "Numpad6" => 77,
        "NumpadAdd" => 78,
        "Numpad1" => 79,
        "Numpad2" => 80,
        "Numpad3" => 81,
        "Numpad0" => 82,
        "NumpadDecimal" => 83,
        "IntlBackslash" => 86,
        "F11" => 87,
        "F12" => 88,
        "IntlRo" => 89,
        "Convert" => 92,
        "KanaMode" => 93,
        "NonConvert" => 94,
        "NumpadEnter" => 96,
        "ControlRight" => 97,
        "NumpadDivide" => 98,
        "PrintScreen" => 99,
        "AltRight" => 100,
        "Home" => 102,
        "ArrowUp" => 103,
        "PageUp" => 104,
        "ArrowLeft" => 105,
        "ArrowRight" => 106,
        "End" => 107,
        "ArrowDown" => 108,
        "PageDown" => 109,
        "Insert" => 110,
        "Delete" => 111,
        "Pause" => 119,
        "SuperLeft" => 125,
        "SuperRight" => 126,
        "ContextMenu" => 127,
        "Help" => 138,
        "Lang1" => 122,
        "Lang2" => 123,
        "Lang3" => 90,
        "Lang4" => 91,
        "Lang5" => 85,
        "NumpadComma" => 121,
        "NumpadEqual" => 117,
        _ => {
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            match c {
                'a'..='z' => 30 + (c as u16 - b'a' as u16),
                'A'..='Z' => 30 + (c as u16 - b'A' as u16),
                '1' => 2,
                '2' => 3,
                '3' => 4,
                '4' => 5,
                '5' => 6,
                '6' => 7,
                '7' => 8,
                '8' => 9,
                '9' => 10,
                '0' => 11,
                ' ' => 57,
                '\t' => 15,
                '\n' => 28,
                '\r' => 28,
                '-' => 12,
                '=' => 13,
                '[' => 26,
                ']' => 27,
                '\\' => 43,
                ';' => 39,
                '\'' => 40,
                '`' => 41,
                ',' => 51,
                '.' => 52,
                '/' => 53,
                _ => return None,
            }
        }
    })
}

/// Parse a mouse button reference from an action string.
///
/// Action strings look like "press mouse 1" or "release mouse 3".
/// Returns the wire-protocol button number.
pub fn parse_mouse_button(name: &str) -> Option<u8> {
    let num = name.strip_prefix("mouse ")?.trim();
    num.parse().ok()
}
