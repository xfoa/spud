//! Bidirectional translation between Linux evdev scancodes and macOS CGKeyCodes.
//!
//! The wire protocol uses evdev scancodes, so:
//! - Server injection (macOS): evdev -> CGKeyCode
//! - Client capture (macOS): CGKeyCode -> evdev
//!
//! Mappings are based on physical key positions on a US-layout keyboard.
//! Keys without a macOS equivalent return None.

use core_graphics::event::KeyCode;

/// Map a Linux evdev scancode to a macOS CGKeyCode.
pub fn evdev_to_macos(code: u16) -> Option<u16> {
    Some(match code {
        1 => KeyCode::ESCAPE,
        2 => KeyCode::ANSI_1,
        3 => KeyCode::ANSI_2,
        4 => KeyCode::ANSI_3,
        5 => KeyCode::ANSI_4,
        6 => KeyCode::ANSI_5,
        7 => KeyCode::ANSI_6,
        8 => KeyCode::ANSI_7,
        9 => KeyCode::ANSI_8,
        10 => KeyCode::ANSI_9,
        11 => KeyCode::ANSI_0,
        12 => KeyCode::ANSI_MINUS,
        13 => KeyCode::ANSI_EQUAL,
        14 => KeyCode::DELETE,
        15 => KeyCode::TAB,
        16 => KeyCode::ANSI_Q,
        17 => KeyCode::ANSI_W,
        18 => KeyCode::ANSI_E,
        19 => KeyCode::ANSI_R,
        20 => KeyCode::ANSI_T,
        21 => KeyCode::ANSI_Y,
        22 => KeyCode::ANSI_U,
        23 => KeyCode::ANSI_I,
        24 => KeyCode::ANSI_O,
        25 => KeyCode::ANSI_P,
        26 => KeyCode::ANSI_LEFT_BRACKET,
        27 => KeyCode::ANSI_RIGHT_BRACKET,
        28 => KeyCode::RETURN,
        29 => KeyCode::CONTROL,
        30 => KeyCode::ANSI_A,
        31 => KeyCode::ANSI_S,
        32 => KeyCode::ANSI_D,
        33 => KeyCode::ANSI_F,
        34 => KeyCode::ANSI_G,
        35 => KeyCode::ANSI_H,
        36 => KeyCode::ANSI_J,
        37 => KeyCode::ANSI_K,
        38 => KeyCode::ANSI_L,
        39 => KeyCode::ANSI_SEMICOLON,
        40 => KeyCode::ANSI_QUOTE,
        41 => KeyCode::ANSI_GRAVE,
        42 => KeyCode::SHIFT,
        43 => KeyCode::ANSI_BACKSLASH,
        44 => KeyCode::ANSI_Z,
        45 => KeyCode::ANSI_X,
        46 => KeyCode::ANSI_C,
        47 => KeyCode::ANSI_V,
        48 => KeyCode::ANSI_B,
        49 => KeyCode::ANSI_N,
        50 => KeyCode::ANSI_M,
        51 => KeyCode::ANSI_COMMA,
        52 => KeyCode::ANSI_PERIOD,
        53 => KeyCode::ANSI_SLASH,
        54 => KeyCode::RIGHT_SHIFT,
        55 => KeyCode::ANSI_KEYPAD_MULTIPLY,
        56 => KeyCode::OPTION,
        57 => KeyCode::SPACE,
        58 => KeyCode::CAPS_LOCK,
        59 => KeyCode::F1,
        60 => KeyCode::F2,
        61 => KeyCode::F3,
        62 => KeyCode::F4,
        63 => KeyCode::F5,
        64 => KeyCode::F6,
        65 => KeyCode::F7,
        66 => KeyCode::F8,
        67 => KeyCode::F9,
        68 => KeyCode::F10,
        71 => KeyCode::ANSI_KEYPAD_7,
        72 => KeyCode::ANSI_KEYPAD_8,
        73 => KeyCode::ANSI_KEYPAD_9,
        74 => KeyCode::ANSI_KEYPAD_MINUS,
        75 => KeyCode::ANSI_KEYPAD_4,
        76 => KeyCode::ANSI_KEYPAD_5,
        77 => KeyCode::ANSI_KEYPAD_6,
        78 => KeyCode::ANSI_KEYPAD_PLUS,
        79 => KeyCode::ANSI_KEYPAD_1,
        80 => KeyCode::ANSI_KEYPAD_2,
        81 => KeyCode::ANSI_KEYPAD_3,
        82 => KeyCode::ANSI_KEYPAD_0,
        83 => KeyCode::ANSI_KEYPAD_DECIMAL,
        86 => KeyCode::ISO_SECTION,
        87 => KeyCode::F11,
        88 => KeyCode::F12,
        96 => KeyCode::ANSI_KEYPAD_ENTER,
        97 => KeyCode::RIGHT_CONTROL,
        98 => KeyCode::ANSI_KEYPAD_DIVIDE,
        100 => KeyCode::RIGHT_OPTION,
        102 => KeyCode::HOME,
        103 => KeyCode::UP_ARROW,
        104 => KeyCode::PAGE_UP,
        105 => KeyCode::LEFT_ARROW,
        106 => KeyCode::RIGHT_ARROW,
        107 => KeyCode::END,
        108 => KeyCode::DOWN_ARROW,
        109 => KeyCode::PAGE_DOWN,
        111 => KeyCode::FORWARD_DELETE,
        117 => KeyCode::ANSI_KEYPAD_EQUAL,
        121 => KeyCode::JIS_KEYPAD_COMMA,
        122 => KeyCode::JIS_EISU,
        125 => KeyCode::COMMAND,
        126 => KeyCode::RIGHT_COMMAND,
        138 => KeyCode::HELP,
        _ => return None,
    })
}

/// Map a macOS CGKeyCode to a Linux evdev scancode.
pub fn macos_to_evdev(code: u16) -> Option<u16> {
    Some(match code {
        KeyCode::ESCAPE => 1,
        KeyCode::ANSI_1 => 2,
        KeyCode::ANSI_2 => 3,
        KeyCode::ANSI_3 => 4,
        KeyCode::ANSI_4 => 5,
        KeyCode::ANSI_5 => 6,
        KeyCode::ANSI_6 => 7,
        KeyCode::ANSI_7 => 8,
        KeyCode::ANSI_8 => 9,
        KeyCode::ANSI_9 => 10,
        KeyCode::ANSI_0 => 11,
        KeyCode::ANSI_MINUS => 12,
        KeyCode::ANSI_EQUAL => 13,
        KeyCode::DELETE => 14,
        KeyCode::TAB => 15,
        KeyCode::ANSI_Q => 16,
        KeyCode::ANSI_W => 17,
        KeyCode::ANSI_E => 18,
        KeyCode::ANSI_R => 19,
        KeyCode::ANSI_T => 20,
        KeyCode::ANSI_Y => 21,
        KeyCode::ANSI_U => 22,
        KeyCode::ANSI_I => 23,
        KeyCode::ANSI_O => 24,
        KeyCode::ANSI_P => 25,
        KeyCode::ANSI_LEFT_BRACKET => 26,
        KeyCode::ANSI_RIGHT_BRACKET => 27,
        KeyCode::RETURN => 28,
        KeyCode::CONTROL => 29,
        KeyCode::ANSI_A => 30,
        KeyCode::ANSI_S => 31,
        KeyCode::ANSI_D => 32,
        KeyCode::ANSI_F => 33,
        KeyCode::ANSI_G => 34,
        KeyCode::ANSI_H => 35,
        KeyCode::ANSI_J => 36,
        KeyCode::ANSI_K => 37,
        KeyCode::ANSI_L => 38,
        KeyCode::ANSI_SEMICOLON => 39,
        KeyCode::ANSI_QUOTE => 40,
        KeyCode::ANSI_GRAVE => 41,
        KeyCode::SHIFT => 42,
        KeyCode::ANSI_BACKSLASH => 43,
        KeyCode::ANSI_Z => 44,
        KeyCode::ANSI_X => 45,
        KeyCode::ANSI_C => 46,
        KeyCode::ANSI_V => 47,
        KeyCode::ANSI_B => 48,
        KeyCode::ANSI_N => 49,
        KeyCode::ANSI_M => 50,
        KeyCode::ANSI_COMMA => 51,
        KeyCode::ANSI_PERIOD => 52,
        KeyCode::ANSI_SLASH => 53,
        KeyCode::RIGHT_SHIFT => 54,
        KeyCode::ANSI_KEYPAD_MULTIPLY => 55,
        KeyCode::OPTION => 56,
        KeyCode::SPACE => 57,
        KeyCode::CAPS_LOCK => 58,
        KeyCode::F1 => 59,
        KeyCode::F2 => 60,
        KeyCode::F3 => 61,
        KeyCode::F4 => 62,
        KeyCode::F5 => 63,
        KeyCode::F6 => 64,
        KeyCode::F7 => 65,
        KeyCode::F8 => 66,
        KeyCode::F9 => 67,
        KeyCode::F10 => 68,
        KeyCode::ANSI_KEYPAD_7 => 71,
        KeyCode::ANSI_KEYPAD_8 => 72,
        KeyCode::ANSI_KEYPAD_9 => 73,
        KeyCode::ANSI_KEYPAD_MINUS => 74,
        KeyCode::ANSI_KEYPAD_4 => 75,
        KeyCode::ANSI_KEYPAD_5 => 76,
        KeyCode::ANSI_KEYPAD_6 => 77,
        KeyCode::ANSI_KEYPAD_PLUS => 78,
        KeyCode::ANSI_KEYPAD_1 => 79,
        KeyCode::ANSI_KEYPAD_2 => 80,
        KeyCode::ANSI_KEYPAD_3 => 81,
        KeyCode::ANSI_KEYPAD_0 => 82,
        KeyCode::ANSI_KEYPAD_DECIMAL => 83,
        KeyCode::ISO_SECTION => 86,
        KeyCode::F11 => 87,
        KeyCode::F12 => 88,
        KeyCode::ANSI_KEYPAD_ENTER => 96,
        KeyCode::RIGHT_CONTROL => 97,
        KeyCode::ANSI_KEYPAD_DIVIDE => 98,
        KeyCode::RIGHT_OPTION => 100,
        KeyCode::HOME => 102,
        KeyCode::UP_ARROW => 103,
        KeyCode::PAGE_UP => 104,
        KeyCode::LEFT_ARROW => 105,
        KeyCode::RIGHT_ARROW => 106,
        KeyCode::END => 107,
        KeyCode::DOWN_ARROW => 108,
        KeyCode::PAGE_DOWN => 109,
        KeyCode::FORWARD_DELETE => 111,
        KeyCode::ANSI_KEYPAD_EQUAL => 117,
        KeyCode::JIS_KEYPAD_COMMA => 121,
        KeyCode::JIS_EISU => 122,
        KeyCode::COMMAND => 125,
        KeyCode::RIGHT_COMMAND => 126,
        KeyCode::HELP => 138,
        _ => return None,
    })
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_evdev_to_macos() {
        for code in 0..=0xFFu16 {
            if let Some(macos) = evdev_to_macos(code) {
                assert_eq!(
                    macos_to_evdev(macos),
                    Some(code),
                    "roundtrip failed for evdev {}",
                    code
                );
            }
        }
    }

    #[test]
    fn roundtrip_macos_to_evdev() {
        for code in 0..=0x7Fu16 {
            if let Some(evdev) = macos_to_evdev(code) {
                assert_eq!(
                    evdev_to_macos(evdev),
                    Some(code),
                    "roundtrip failed for macos {}",
                    code
                );
            }
        }
    }

    #[test]
    fn coverage_common_keys() {
        let common = [
            1, 2, 3, 14, 15, 28, 29, 30, 42, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 87,
            88, 103, 105, 106, 108, 125, 126,
        ];
        for &code in &common {
            assert!(
                evdev_to_macos(code).is_some(),
                "common key {} missing macOS mapping",
                code
            );
        }
    }
}
