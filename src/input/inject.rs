use std::io;
use std::sync::mpsc::{self, Sender as MpscSender};
use std::thread::{self, JoinHandle};

/// Commands sent to the injector worker thread.
enum InjectCmd {
    MouseAbs { x: i32, y: i32 },
    MouseRel { dx: i32, dy: i32 },
    KeyDown { code: u16 },
    KeyUp { code: u16 },
    ButtonDown { code: u16 },
    ButtonUp { code: u16 },
    Wheel { dx: i8, dy: i8 },
}

/// Injects input events into the host via Linux uinput.
///
/// Mouse movement uses `kinput` (already working well).
/// Keyboard, mouse buttons, and wheel use `evdev::VirtualDevice`.
pub struct InputInjector {
    tx: MpscSender<InjectCmd>,
    _handle: JoinHandle<()>,
}

impl InputInjector {
    /// Create a new injector for the given screen dimensions.
    ///
    /// The absolute mouse device is configured with `screen_width` x `screen_height`
    /// so that normalized 0..65535 wire coordinates map to the full screen.
    pub fn new(screen_width: u16, screen_height: u16) -> io::Result<Self> {
        // Pre-check /dev/uinput so we can return a clean error instead of
        // letting kinput panic.
        match std::fs::OpenOptions::new().write(true).open("/dev/uinput") {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Permission denied opening /dev/uinput. \
                     Add your user to the 'input' group and re-login, \
                     or create a udev rule:\
                     echo 'KERNEL=\"uinput\", MODE=\"0660\", GROUP=\"input\"' \
                     | sudo tee /etc/udev/rules.d/99-uinput.rules && \
                     sudo udevadm control --reload-rules && \
                     sudo udevadm trigger"
                ));
            }
            Err(e) => return Err(e),
        }

        let (tx, rx) = mpsc::channel::<InjectCmd>();

        let handle = thread::spawn(move || {
            // Create kinput device for mouse movement.
            let kinput_device = kinput::InputDevice::from((
                i32::from(screen_width),
                i32::from(screen_height),
                kinput::Layout::Us,
            ));
            println!("[spud] kinput device created ({}x{})", screen_width, screen_height);

            // Create evdev virtual device for keys, buttons, and wheel.
            let mut evdev_device = match create_evdev_device() {
                Ok(dev) => {
                    println!("[spud] evdev virtual device created");
                    Some(dev)
                }
                Err(e) => {
                    eprintln!("[spud] failed to create evdev virtual device: {e}");
                    None
                }
            };

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    InjectCmd::MouseAbs { x, y } => {
                        kinput_device.mouse.abs.move_xy(x, y);
                    }
                    InjectCmd::MouseRel { dx, dy } => {
                        kinput_device.mouse.rel.move_xy(dx, dy);
                    }
                    InjectCmd::KeyDown { code } => {
                        if let Some(ref mut dev) = evdev_device {
                            let _ = emit_key(dev, code, 1);
                        }
                    }
                    InjectCmd::KeyUp { code } => {
                        if let Some(ref mut dev) = evdev_device {
                            let _ = emit_key(dev, code, 0);
                        }
                    }
                    InjectCmd::ButtonDown { code } => {
                        if let Some(ref mut dev) = evdev_device {
                            let _ = emit_key(dev, code, 1);
                        }
                    }
                    InjectCmd::ButtonUp { code } => {
                        if let Some(ref mut dev) = evdev_device {
                            let _ = emit_key(dev, code, 0);
                        }
                    }
                    InjectCmd::Wheel { dx, dy } => {
                        if let Some(ref mut dev) = evdev_device {
                            let _ = emit_wheel(dev, dx, dy);
                        }
                    }
                }
            }
            println!("[spud] input injector thread exiting");
        });

        Ok(Self { tx, _handle: handle })
    }

    /// Move the cursor to an absolute screen position (pixels).
    pub fn move_abs(&self, x: i32, y: i32) {
        let _ = self.tx.send(InjectCmd::MouseAbs { x, y });
    }

    /// Move the cursor by a relative delta (pixels).
    pub fn move_rel(&self, dx: i32, dy: i32) {
        let _ = self.tx.send(InjectCmd::MouseRel { dx, dy });
    }

    /// Press a keyboard key by Linux evdev keycode.
    pub fn key_down(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::KeyDown { code });
    }

    /// Release a keyboard key by Linux evdev keycode.
    pub fn key_up(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::KeyUp { code });
    }

    /// Press a mouse button by Linux evdev button code.
    pub fn button_down(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::ButtonDown { code });
    }

    /// Release a mouse button by Linux evdev button code.
    pub fn button_up(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::ButtonUp { code });
    }

    /// Emit a mouse wheel event.
    pub fn wheel(&self, dx: i8, dy: i8) {
        let _ = self.tx.send(InjectCmd::Wheel { dx, dy });
    }

    /// Legacy action parser used by the key tracker for timeout releases.
    pub fn inject_action(&mut self, action: &str) {
        let action = action.trim();
        if let Some(rest) = action.strip_prefix("press ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyDown { code });
            } else if let Some(btn) = parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonDown { code: btn });
            }
        } else if let Some(rest) = action.strip_prefix("release ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyUp { code });
            } else if let Some(btn) = parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonUp { code: btn });
            }
        }
        // repeat actions are ignored for injection (they're just heartbeats)
    }
}

fn create_evdev_device() -> io::Result<evdev::uinput::VirtualDevice> {
    let mut keys = evdev::AttributeSet::<evdev::KeyCode>::new();
    for code in 0..=0x2ffu16 {
        keys.insert(evdev::KeyCode::new(code));
    }

    let mut rel_axes = evdev::AttributeSet::<evdev::RelativeAxisCode>::new();
    rel_axes.insert(evdev::RelativeAxisCode::REL_WHEEL);
    rel_axes.insert(evdev::RelativeAxisCode::REL_HWHEEL);

    evdev::uinput::VirtualDevice::builder()?
        .name(b"spud virtual input")
        .with_keys(&keys)?
        .with_relative_axes(&rel_axes)?
        .build()
}

fn emit_key(dev: &mut evdev::uinput::VirtualDevice, code: u16, value: i32) -> io::Result<()> {
    use evdev::{EventType, InputEvent, KeyCode, SynchronizationCode};
    dev.emit(&[
        InputEvent::new_now(EventType::KEY.0, KeyCode::new(code).0, value),
        InputEvent::new_now(EventType::SYNCHRONIZATION.0, SynchronizationCode::SYN_REPORT.0, 0),
    ])
}

fn emit_wheel(dev: &mut evdev::uinput::VirtualDevice, dx: i8, dy: i8) -> io::Result<()> {
    use evdev::{EventType, InputEvent, RelativeAxisCode, SynchronizationCode};
    let mut events = Vec::with_capacity(3);
    if dy != 0 {
        events.push(InputEvent::new_now(
            EventType::RELATIVE.0,
            RelativeAxisCode::REL_WHEEL.0,
            i32::from(dy),
        ));
    }
    if dx != 0 {
        events.push(InputEvent::new_now(
            EventType::RELATIVE.0,
            RelativeAxisCode::REL_HWHEEL.0,
            i32::from(dx),
        ));
    }
    events.push(InputEvent::new_now(
        EventType::SYNCHRONIZATION.0,
        SynchronizationCode::SYN_REPORT.0,
        0,
    ));
    dev.emit(&events)
}

/// Parse a key name sent by the client into a Linux evdev keycode.
///
/// Supports:
/// - `evdev:N` format (raw scancode)
/// - Common logical names like "Space", "Enter", "Escape", etc.
pub fn parse_key_name(name: &str) -> Option<u16> {
    if let Some(num) = name.strip_prefix("evdev:") {
        return num.parse::<u16>().ok();
    }

    // Common logical key names from iced::keyboard::key::Named
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
            // Try single-character ASCII mapping (best-effort, US-layout biased)
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None; // multi-char, not handled
            }
            match c {
                'a'..='z' => 30 + (c as u16 - b'a' as u16),
                'A'..='Z' => 30 + (c as u16 - b'A' as u16),
                '1' => 2, '2' => 3, '3' => 4, '4' => 5, '5' => 6,
                '6' => 7, '7' => 8, '8' => 9, '9' => 10, '0' => 11,
                ' ' => 57,
                '\t' => 15,
                '\n' => 28,
                '\r' => 28,
                '-' => 12, '=' => 13, '[' => 26, ']' => 27,
                '\\' => 43, ';' => 39, '\'' => 40, '`' => 41,
                ',' => 51, '.' => 52, '/' => 53,
                _ => return None,
            }
        }
    })
}

/// Parse a mouse button reference from an action string.
///
/// Action strings look like "press mouse 1" or "release mouse 3".
fn parse_mouse_button(name: &str) -> Option<u16> {
    let num = name.strip_prefix("mouse ")?.trim();
    let wire: u8 = num.parse().ok()?;
    Some(wire_to_linux_button(wire))
}

/// Convert a wire-protocol mouse button number to a Linux evdev button code.
pub fn wire_to_linux_button(wire: u8) -> u16 {
    match wire {
        1 => 0x110, // BTN_LEFT
        2 => 0x112, // BTN_MIDDLE
        3 => 0x111, // BTN_RIGHT
        8 => 0x113, // BTN_SIDE
        9 => 0x114, // BTN_EXTRA
        other => 0x110 + u16::from(other.saturating_sub(1)), // fallback
    }
}
