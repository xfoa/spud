use std::cell::Cell;
use std::error::Error;
use std::thread;

use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField, KeyCode,
};
use iced::futures::channel::mpsc;
use iced::futures::Stream;

use crate::input::InputEvent;

pub fn listen(hotkey: String) -> impl Stream<Item = InputEvent> + Send + 'static {
    iced::stream::channel(256, move |mut output| async move {
        let hotkey = hotkey.clone();
        thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel::<InputEvent>();

            // Forward events from the synchronous tap callback to the async output.
            thread::spawn(move || {
                while let Ok(event) = rx.recv() {
                    if output.try_send(event).is_err() {
                        break;
                    }
                }
            });

            if let Err(e) = run(&hotkey, tx) {
                eprintln!("[spud] macOS input backend stopped: {e}");
            }
        });
    })
}

fn run(
    hotkey: &str,
    tx: std::sync::mpsc::Sender<InputEvent>,
) -> Result<(), Box<dyn Error>> {
    let (hotkey_mods, hotkey_key) = match parse_hotkey(hotkey) {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(InputEvent::BackendError(format!(
                "could not parse hotkey '{hotkey}': {e}"
            )));
            return Ok(());
        }
    };

    let relevant_flags = CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagCommand;

    let grabbed = Cell::new(false);

    let events = vec![
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGEventType::RightMouseDown,
        CGEventType::RightMouseUp,
        CGEventType::OtherMouseDown,
        CGEventType::OtherMouseUp,
        CGEventType::MouseMoved,
        CGEventType::ScrollWheel,
    ];

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        events,
        move |_proxy, event_type, event| {
            use core_graphics::event::CallbackResult;

            // Re-enable the tap if the system disabled it.
            if matches!(
                event_type,
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
            ) {
                return CallbackResult::Keep;
            }

            // Hotkey detection (only on keydown).
            if matches!(event_type, CGEventType::KeyDown) {
                let keycode =
                    event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                let flags = event.get_flags() & relevant_flags;
                if keycode == hotkey_key && flags == hotkey_mods {
                    let new_grabbed = !grabbed.get();
                    grabbed.set(new_grabbed);
                    let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(!new_grabbed);
                    if new_grabbed {
                        let _ = CGDisplay::main().hide_cursor();
                    } else {
                        let _ = CGDisplay::main().show_cursor();
                    }
                    let _ = tx.send(InputEvent::HotkeyToggled {
                        grabbed: new_grabbed,
                    });
                    return CallbackResult::Drop;
                }
            }

            if !grabbed.get() {
                return CallbackResult::Keep;
            }

            // When grabbed, translate events and send them to the remote.
            let input_event = match event_type {
                CGEventType::KeyDown => {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u8;
                    Some(InputEvent::KeyPress { keycode })
                }
                CGEventType::KeyUp => {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u8;
                    Some(InputEvent::KeyRelease { keycode })
                }
                CGEventType::LeftMouseDown => {
                    Some(InputEvent::MouseButton { button: 1, pressed: true })
                }
                CGEventType::LeftMouseUp => {
                    Some(InputEvent::MouseButton { button: 1, pressed: false })
                }
                CGEventType::RightMouseDown => {
                    Some(InputEvent::MouseButton { button: 3, pressed: true })
                }
                CGEventType::RightMouseUp => {
                    Some(InputEvent::MouseButton { button: 3, pressed: false })
                }
                CGEventType::OtherMouseDown => {
                    let btn = event
                        .get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER)
                        as u8;
                    Some(InputEvent::MouseButton {
                        button: macos_button_to_wire(btn),
                        pressed: true,
                    })
                }
                CGEventType::OtherMouseUp => {
                    let btn = event
                        .get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER)
                        as u8;
                    Some(InputEvent::MouseButton {
                        button: macos_button_to_wire(btn),
                        pressed: false,
                    })
                }
                CGEventType::MouseMoved => {
                    let dx = event
                        .get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X)
                        as i16;
                    let dy = event
                        .get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y)
                        as i16;
                    Some(InputEvent::MouseMove { dx, dy })
                }
                CGEventType::ScrollWheel => {
                    let dy = event
                        .get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1)
                        as i8;
                    let dx = event
                        .get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2)
                        as i8;
                    Some(InputEvent::Wheel { dx, dy })
                }
                _ => None,
            };

            if let Some(ie) = input_event {
                let _ = tx.send(ie);
            }

            // Drop all grabbed events so they don't reach local applications.
            CallbackResult::Drop
        },
    );

    let tap = match tap {
        Ok(t) => t,
        Err(_) => {
            let _ = tx.send(InputEvent::BackendError(
                "failed to create CGEventTap. Accessibility permission may be required."
                    .to_string(),
            ));
            return Ok(());
        }
    };

    let runloop = core_foundation::runloop::CFRunLoop::get_current();
    let source = tap
        .mach_port()
        .create_runloop_source(0)
        .expect("runloop source creation failed");
    runloop.add_source(&source, unsafe { core_foundation::runloop::kCFRunLoopCommonModes });
    tap.enable();

    // Run until the process exits. There is no clean cancellation path
    // for a CFRunLoop without additional machinery (a timer or port signal).
    core_foundation::runloop::CFRunLoop::run_current();

    Ok(())
}

fn macos_button_to_wire(macos_btn: u8) -> u8 {
    match macos_btn {
        0 => 1,  // left
        1 => 3,  // right
        2 => 2,  // center
        3 => 8,  // back
        4 => 9,  // forward
        other => other,
    }
}

fn parse_hotkey(hotkey: &str) -> Result<(CGEventFlags, u16), String> {
    let mut modifiers = CGEventFlags::CGEventFlagNull;
    let mut key_label: Option<&str> = None;

    for part in hotkey.split('+').map(|p| p.trim()) {
        match part {
            "Ctrl" => modifiers |= CGEventFlags::CGEventFlagControl,
            "Alt" => modifiers |= CGEventFlags::CGEventFlagAlternate,
            "Shift" => modifiers |= CGEventFlags::CGEventFlagShift,
            "Super" | "Meta" | "Cmd" | "Command" => {
                modifiers |= CGEventFlags::CGEventFlagCommand
            }
            other => {
                if key_label.is_some() {
                    return Err(format!("multiple non-modifier keys in '{hotkey}'"));
                }
                key_label = Some(other);
            }
        }
    }

    let label = key_label.ok_or_else(|| "no non-modifier key in hotkey".to_string())?;
    let keycode = label_to_macos_keycode(label)
        .ok_or_else(|| format!("unsupported key '{label}'"))?;

    Ok((modifiers, keycode))
}

fn label_to_macos_keycode(label: &str) -> Option<u16> {
    Some(match label {
        "Space" => KeyCode::SPACE,
        "Enter" => KeyCode::RETURN,
        "Tab" => KeyCode::TAB,
        "Backspace" => KeyCode::DELETE,
        "Delete" => KeyCode::FORWARD_DELETE,
        "Home" => KeyCode::HOME,
        "End" => KeyCode::END,
        "Page Up" => KeyCode::PAGE_UP,
        "Page Down" => KeyCode::PAGE_DOWN,
        "Left" => KeyCode::LEFT_ARROW,
        "Right" => KeyCode::RIGHT_ARROW,
        "Up" => KeyCode::UP_ARROW,
        "Down" => KeyCode::DOWN_ARROW,
        "Caps Lock" => KeyCode::CAPS_LOCK,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        other => {
            let mut chars = other.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            match c {
                'a'..='z' => KeyCode::ANSI_A + (c as u16 - b'a' as u16),
                'A'..='Z' => KeyCode::ANSI_A + (c as u16 - b'A' as u16),
                '1' => KeyCode::ANSI_1,
                '2' => KeyCode::ANSI_2,
                '3' => KeyCode::ANSI_3,
                '4' => KeyCode::ANSI_4,
                '5' => KeyCode::ANSI_5,
                '6' => KeyCode::ANSI_6,
                '7' => KeyCode::ANSI_7,
                '8' => KeyCode::ANSI_8,
                '9' => KeyCode::ANSI_9,
                '0' => KeyCode::ANSI_0,
                '`' => KeyCode::ANSI_GRAVE,
                '-' => KeyCode::ANSI_MINUS,
                '=' => KeyCode::ANSI_EQUAL,
                '[' => KeyCode::ANSI_LEFT_BRACKET,
                ']' => KeyCode::ANSI_RIGHT_BRACKET,
                '\\' => KeyCode::ANSI_BACKSLASH,
                ';' => KeyCode::ANSI_SEMICOLON,
                '\'' => KeyCode::ANSI_QUOTE,
                ',' => KeyCode::ANSI_COMMA,
                '.' => KeyCode::ANSI_PERIOD,
                '/' => KeyCode::ANSI_SLASH,
                _ => return None,
            }
        }
    })
}
