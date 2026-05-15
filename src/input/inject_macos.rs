use std::io;
use std::sync::mpsc::{self, Sender as MpscSender};
use std::thread::{self, JoinHandle};

use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventType, CGMouseButton, EventField,
};
use core_graphics::geometry::CGPoint;

use crate::input::key_names;
use crate::input::macos_keycodes;

/// Commands sent to the injector worker thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InjectCmd {
    MouseAbs { x: i32, y: i32 },
    MouseRel { dx: i32, dy: i32 },
    KeyDown { code: u16 },
    KeyUp { code: u16 },
    ButtonDown { code: u16 },
    ButtonUp { code: u16 },
    Wheel { dx: i8, dy: i8 },
}

/// Injects input events into macOS via Core Graphics.
pub struct InputInjector {
    tx: MpscSender<InjectCmd>,
    _handle: JoinHandle<()>,
}

impl InputInjector {
    pub fn new(screen_width: u16, screen_height: u16) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel::<InjectCmd>();

        let handle = thread::spawn(move || {
            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("[spud] Failed to create CGEventSource");
                    return;
                }
            };

            // Track cursor position for relative movement.
            let mut cursor = get_cursor_position(&source).unwrap_or_else(|| {
                CGPoint::new(
                    f64::from(screen_width) / 2.0,
                    f64::from(screen_height) / 2.0,
                )
            });

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    InjectCmd::MouseAbs { x, y } => {
                        cursor = CGPoint::new(f64::from(x), f64::from(y));
                        if let Ok(event) = CGEvent::new_mouse_event(
                            source.clone(),
                            CGEventType::MouseMoved,
                            cursor,
                            CGMouseButton::Left,
                        ) {
                            event.post(CGEventTapLocation::HID);
                        }
                    }
                    InjectCmd::MouseRel { dx, dy } => {
                        cursor.x += f64::from(dx);
                        cursor.y += f64::from(dy);
                        // Clamp to screen bounds.
                        let main = CGDisplay::main();
                        let bounds = main.bounds();
                        cursor.x = cursor.x.clamp(bounds.origin.x, bounds.origin.x + bounds.size.width);
                        cursor.y = cursor.y.clamp(bounds.origin.y, bounds.origin.y + bounds.size.height);
                        if let Ok(event) = CGEvent::new_mouse_event(
                            source.clone(),
                            CGEventType::MouseMoved,
                            cursor,
                            CGMouseButton::Left,
                        ) {
                            event.post(CGEventTapLocation::HID);
                        }
                    }
                    InjectCmd::KeyDown { code } => {
                        if let Some(keycode) = macos_keycodes::evdev_to_macos(code) {
                            if let Ok(event) =
                                CGEvent::new_keyboard_event(source.clone(), keycode, true)
                            {
                                event.post(CGEventTapLocation::HID);
                            }
                        } else {
                            eprintln!("[spud] No macOS keycode for evdev {code}");
                        }
                    }
                    InjectCmd::KeyUp { code } => {
                        if let Some(keycode) = macos_keycodes::evdev_to_macos(code) {
                            if let Ok(event) =
                                CGEvent::new_keyboard_event(source.clone(), keycode, false)
                            {
                                event.post(CGEventTapLocation::HID);
                            }
                        } else {
                            eprintln!("[spud] No macOS keycode for evdev {code}");
                        }
                    }
                    InjectCmd::ButtonDown { code } => {
                        post_mouse_button(&source, &mut cursor, code, true);
                    }
                    InjectCmd::ButtonUp { code } => {
                        post_mouse_button(&source, &mut cursor, code, false);
                    }
                    InjectCmd::Wheel { dx, dy } => {
                        post_scroll(&source, dx, dy);
                    }
                }
            }
            eprintln!("[spud] macOS input injector thread exiting");
        });

        Ok(Self { tx, _handle: handle })
    }

    pub fn move_abs(&self, x: i32, y: i32) {
        let _ = self.tx.send(InjectCmd::MouseAbs { x, y });
    }

    pub fn move_rel(&self, dx: i32, dy: i32) {
        let _ = self.tx.send(InjectCmd::MouseRel { dx, dy });
    }

    pub fn key_down(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::KeyDown { code });
    }

    pub fn key_up(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::KeyUp { code });
    }

    pub fn button_down(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::ButtonDown { code });
    }

    pub fn button_up(&self, code: u16) {
        let _ = self.tx.send(InjectCmd::ButtonUp { code });
    }

    pub fn wheel(&self, dx: i8, dy: i8) {
        let _ = self.tx.send(InjectCmd::Wheel { dx, dy });
    }

    pub fn inject_action(&self, action: &str) {
        let action = action.trim();
        if let Some(rest) = action.strip_prefix("press ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = key_names::parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyDown { code });
            } else if let Some(btn) = key_names::parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonDown { code: btn as u16 });
            }
        } else if let Some(rest) = action.strip_prefix("release ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = key_names::parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyUp { code });
            } else if let Some(btn) = key_names::parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonUp { code: btn as u16 });
            }
        }
    }
}

fn get_cursor_position(source: &CGEventSource) -> Option<CGPoint> {
    let event = CGEvent::new(source.clone()).ok()?;
    Some(event.location())
}

fn post_mouse_button(source: &CGEventSource, cursor: &mut CGPoint, wire: u16, pressed: bool) {
    let wire = wire as u8;
    let (event_type, button) = match (wire, pressed) {
        (1, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
        (1, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
        (3, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
        (3, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
        (2, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
        (2, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        _ => {
            // Back/forward buttons (and any others) use OtherMouse with a raw button number.
            let raw_button = wire as i64;
            let event_type = if pressed {
                CGEventType::OtherMouseDown
            } else {
                CGEventType::OtherMouseUp
            };
            if let Ok(event) = CGEvent::new(source.clone()) {
                event.set_type(event_type);
                event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, raw_button);
                event.set_location(*cursor);
                event.post(CGEventTapLocation::HID);
            }
            return;
        }
    };

    if let Ok(event) = CGEvent::new_mouse_event(source.clone(), event_type, *cursor, button) {
        event.post(CGEventTapLocation::HID);
    }
}

fn post_scroll(source: &CGEventSource, dx: i8, dy: i8) {
    if let Ok(event) = CGEvent::new(source.clone()) {
        event.set_type(CGEventType::ScrollWheel);
        // Axis 1 = vertical, Axis 2 = horizontal
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, i64::from(dy));
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2, i64::from(dx));
        event.post(CGEventTapLocation::HID);
    }
}
