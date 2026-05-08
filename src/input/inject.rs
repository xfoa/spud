use evdev::{uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use std::io;

/// Injects keyboard and mouse events into the host via Linux uinput.
pub struct InputInjector {
    device: VirtualDevice,
}

impl InputInjector {
    pub fn new() -> io::Result<Self> {
        // Enable all keycodes that could be sent by a client (0..=767 covers
        // the full Linux input keycode range).
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 0..=767u16 {
            keys.insert(KeyCode(code));
        }

        let device = VirtualDevice::builder()?
            .name("Spud Input Injector")
            .with_keys(&keys)?
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .build()?;

        Ok(Self { device })
    }

    /// Inject a key press or release. `code` is the Linux evdev keycode.
    pub fn key(&mut self, code: u16, pressed: bool) -> io::Result<()> {
        let value = if pressed { 1 } else { 0 };
        self.device
            .emit(&[InputEvent::new(EventType::KEY.0, code, value)])
    }

    /// Inject a mouse button press or release.
    /// `button` uses the wire-protocol logical codes (1=left, 2=middle,
    /// 3=right, 4=side, 5=extra).
    pub fn mouse_button(&mut self, button: u8, pressed: bool) -> io::Result<()> {
        let code = match button {
            1 => 0x110, // BTN_LEFT
            2 => 0x112, // BTN_MIDDLE
            3 => 0x111, // BTN_RIGHT
            4 => 0x113, // BTN_SIDE
            5 => 0x114, // BTN_EXTRA
            _ => 0x110,
        };
        let value = if pressed { 1 } else { 0 };
        self.device
            .emit(&[InputEvent::new(EventType::KEY.0, code, value)])
    }

    /// Inject relative mouse movement.
    pub fn mouse_move(&mut self, dx: i32, dy: i32) -> io::Result<()> {
        self.device.emit(&[
            InputEvent::new_now(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, dx),
            InputEvent::new_now(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, dy),
        ])
    }

    /// Inject scroll wheel events.
    ///
    /// Sign convention: `dy > 0` = scroll down, `dx > 0` = scroll right.
    /// evdev REL_WHEEL uses the opposite convention for vertical (positive = up),
    /// so `dy` is negated. REL_HWHEEL matches our convention (positive = right).
    pub fn wheel(&mut self, dx: i8, dy: i8) -> io::Result<()> {
        let mut events = Vec::new();
        if dy != 0 {
            events.push(InputEvent::new_now(
                EventType::RELATIVE.0,
                RelativeAxisCode::REL_WHEEL.0,
                -i32::from(dy),
            ));
        }
        if dx != 0 {
            events.push(InputEvent::new_now(
                EventType::RELATIVE.0,
                RelativeAxisCode::REL_HWHEEL.0,
                i32::from(dx),
            ));
        }
        if !events.is_empty() {
            self.device.emit(&events)?;
        }
        Ok(())
    }

    /// Parse a tracker action string and inject the corresponding event.
    ///
    /// Supported action formats:
    /// - `press <key>` / `press <key> (repeat without prior down)`
    /// - `release <key>` / `release <key> (lost up)` / `release <key> (timeout)`
    /// - `press mouse <btn>` / `release mouse <btn>` / `release mouse <btn> (lost up)` / `release mouse <btn> (timeout)`
    ///
    /// Repeat actions are ignored because the key is already held down.
    pub fn inject_action(&mut self, action: &str) {
        if let Some(rest) = action.strip_prefix("press ") {
            if let Some(btn_str) = rest.strip_prefix("mouse ") {
                let btn = btn_str.split(" (").next().unwrap_or("");
                if let Ok(button) = btn.parse::<u8>() {
                    let _ = self.mouse_button(button, true);
                }
            } else {
                let name = rest.split(" (").next().unwrap_or("");
                if let Some(code) = name_to_keycode(name) {
                    let _ = self.key(code, true);
                } else {
                    eprintln!("[injector] unknown key name: {name}");
                }
            }
        } else if let Some(rest) = action.strip_prefix("release ") {
            if let Some(btn_str) = rest.strip_prefix("mouse ") {
                let btn = btn_str.split(" (").next().unwrap_or("");
                if let Ok(button) = btn.parse::<u8>() {
                    let _ = self.mouse_button(button, false);
                }
            } else {
                let name = rest.split(" (").next().unwrap_or("");
                if let Some(code) = name_to_keycode(name) {
                    let _ = self.key(code, false);
                } else {
                    eprintln!("[injector] unknown key name: {name}");
                }
            }
        }
        // "repeat ..." actions are intentionally ignored.
    }
}

/// Map a key name to its Linux evdev keycode.
///
/// Supports two formats:
/// - `"evdev:N"` -- raw numeric code (from X11 hotkey mode)
/// - Semantic names like `"a"`, `"Enter"`, `"Space"` (from iced direct mode)
fn name_to_keycode(name: &str) -> Option<u16> {
    // Try evdev: prefix first
    if let Some(num) = name.strip_prefix("evdev:") {
        return num.parse().ok();
    }

    // Semantic names from iced keyboard events
    let code = match name {
        // Letters
        "a" => 30,
        "b" => 48,
        "c" => 46,
        "d" => 32,
        "e" => 18,
        "f" => 33,
        "g" => 34,
        "h" => 35,
        "i" => 23,
        "j" => 36,
        "k" => 37,
        "l" => 38,
        "m" => 50,
        "n" => 49,
        "o" => 24,
        "p" => 25,
        "q" => 16,
        "r" => 19,
        "s" => 31,
        "t" => 20,
        "u" => 22,
        "v" => 47,
        "w" => 17,
        "x" => 45,
        "y" => 21,
        "z" => 44,
        // Digits
        "1" => 2,
        "2" => 3,
        "3" => 4,
        "4" => 5,
        "5" => 6,
        "6" => 7,
        "7" => 8,
        "8" => 9,
        "9" => 10,
        "0" => 11,
        // Named keys
        "Escape" => 1,
        "Tab" => 15,
        "Enter" => 28,
        "Backspace" => 14,
        "Space" => 57,
        "Delete" => 111,
        "Insert" => 110,
        "Home" => 102,
        "End" => 107,
        "PageUp" => 104,
        "PageDown" => 109,
        "ArrowUp" => 103,
        "ArrowDown" => 108,
        "ArrowLeft" => 105,
        "ArrowRight" => 106,
        // Function keys
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
        "F11" => 87,
        "F12" => 88,
        // Modifiers
        "Shift" => 42,
        "Control" => 29,
        "Alt" => 56,
        "Super" => 125,
        "CapsLock" => 58,
        "NumLock" => 69,
        "ScrollLock" => 70,
        // Punctuation (US layout)
        "-" | "_" => 12,
        "=" | "+" => 13,
        "[" | "{" => 26,
        "]" | "}" => 27,
        "\\" | "|" => 43,
        ";" | ":" => 39,
        "'" | "\"" => 40,
        "," | "<" => 51,
        "." | ">" => 52,
        "/" | "?" => 53,
        "`" | "~" => 41,
        _ => return None,
    };
    Some(code)
}
