use std::collections::{HashMap, HashSet};
use std::io;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::mpsc::{self, RecvTimeoutError, Sender as MpscSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, EventField, KeyCode,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
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

/// Injects input events into macOS via Core Graphics + IOKit HID.
pub struct InputInjector {
    tx: MpscSender<InjectCmd>,
    _handle: JoinHandle<()>,
}

/// Delay before the first repeat keydown is generated.z\
/// Delay before the first repeat keydown is generated.
const REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(400);
/// Interval between subsequent repeat keydowns.
const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

impl InputInjector {
    pub fn new(screen_width: u16, screen_height: u16) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel::<InjectCmd>();

        let handle = thread::spawn(move || {
            // Mouse goes through IOKit HID so raw-input games see it.
            let hid = match IoKitHid::open() {
                Some(h) => h,
                None => {
                    eprintln!("[spud] Failed to open IOKit HID connection");
                    return;
                }
            };

            // Scroll and cursor query still need Core Graphics.
            let cg_source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("[spud] Failed to create CGEventSource");
                    return;
                }
            };

            let mut cursor = get_cursor_position(&cg_source).unwrap_or_else(|| {
                CGPoint::new(
                    f64::from(screen_width) / 2.0,
                    f64::from(screen_height) / 2.0,
                )
            });

            let mut pressed_buttons: HashSet<u8> = HashSet::new();
            let mut held_keys: HashMap<u16, Instant> = HashMap::new();

            loop {
                match rx.recv_timeout(REPEAT_INTERVAL) {
                    Ok(cmd) => {
                        match cmd {
                            InjectCmd::MouseAbs { x, y } => {
                                cursor = CGPoint::new(f64::from(x), f64::from(y));
                                post_mouse_move(&hid, cursor, &pressed_buttons);
                            }
                            InjectCmd::MouseRel { dx, dy } => {
                                cursor.x += f64::from(dx);
                                cursor.y += f64::from(dy);
                                // Don't clamp or warp for relative mode — the game/app
                                // handles its own cursor position.  Just post the delta.
                                post_mouse_relative(&hid, dx, dy, &pressed_buttons);
                            }
                            InjectCmd::KeyDown { code } => {
                                if let Some(keycode) = macos_keycodes::evdev_to_macos(code) {
                                    held_keys.insert(code, Instant::now() + REPEAT_INITIAL_DELAY);
                                    if let Ok(event) =
                                        CGEvent::new_keyboard_event(cg_source.clone(), keycode, true)
                                    {
                                        event.post(CGEventTapLocation::HID);
                                    }
                                } else {
                                    eprintln!("[spud] No macOS keycode for evdev {code}");
                                }
                            }
                            InjectCmd::KeyUp { code } => {
                                if let Some(keycode) = macos_keycodes::evdev_to_macos(code) {
                                    held_keys.remove(&code);
                                    if let Ok(event) =
                                        CGEvent::new_keyboard_event(cg_source.clone(), keycode, false)
                                    {
                                        event.post(CGEventTapLocation::HID);
                                    }
                                } else {
                                    eprintln!("[spud] No macOS keycode for evdev {code}");
                                }
                            }
                            InjectCmd::ButtonDown { code } => {
                                pressed_buttons.insert(code as u8);
                                post_mouse_button(&hid, cursor, code, true);
                            }
                            InjectCmd::ButtonUp { code } => {
                                pressed_buttons.remove(&(code as u8));
                                post_mouse_button(&hid, cursor, code, false);
                            }
                            InjectCmd::Wheel { dx, dy } => {
                                post_scroll(&hid, &cg_source, dx, dy);
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                // Pulse key-up / key-down on every repeat tick.
                // We deliberately do NOT set KEYBOARD_EVENT_AUTOREPEAT on the down
                // event — a "fresh" keydown resets macOS's accent-menu timer,
                // whereas an autorepeat keydown is ignored by the Text Input system.
                // Modifier keys are never pulsed.
                let now = Instant::now();
                for (code, next_repeat) in &mut held_keys {
                    if now >= *next_repeat {
                        if let Some(keycode) = macos_keycodes::evdev_to_macos(*code) {
                            let is_modifier = modifier_flag_for_keycode(keycode) != 0;
                            if !is_modifier {
                                if let Ok(up) =
                                    CGEvent::new_keyboard_event(cg_source.clone(), keycode, false)
                                {
                                    up.post(CGEventTapLocation::HID);
                                }
                                if let Ok(down) =
                                    CGEvent::new_keyboard_event(cg_source.clone(), keycode, true)
                                {
                                    down.post(CGEventTapLocation::HID);
                                }
                            }
                        }
                        *next_repeat = now + REPEAT_INTERVAL;
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

/* ------------------------------------------------------------------ */
/* Core Graphics helpers                                              */
/* ------------------------------------------------------------------ */

fn get_cursor_position(source: &CGEventSource) -> Option<CGPoint> {
    let event = CGEvent::new(source.clone()).ok()?;
    Some(event.location())
}

/* ------------------------------------------------------------------ */
/* IOKit FFI                                                          */
/* ------------------------------------------------------------------ */

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(masterPort: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owningTask: u32, r#type: u32, connect: *mut u32) -> c_int;
    fn IOHIDPostEvent(
        connect: u32,
        eventType: u32,
        location: IOGPoint,
        eventData: *const NXEventData,
        eventDataVersion: u32,
        eventFlags: u32,
        options: u32,
    ) -> c_int;
    fn IOObjectRelease(object: u32) -> c_int;
}

extern "C" {
    fn mach_task_self() -> u32;
}

const NX_LMOUSEDOWN: u32 = 1;
const NX_LMOUSEUP: u32 = 2;
const NX_RMOUSEDOWN: u32 = 3;
const NX_RMOUSEUP: u32 = 4;
const NX_MOUSEMOVED: u32 = 5;
const NX_LMOUSEDRAGGED: u32 = 6;
const NX_RMOUSEDRAGGED: u32 = 7;
const NX_OMOUSEDOWN: u32 = 25;
const NX_OMOUSEUP: u32 = 26;
const NX_OMOUSEDRAGGED: u32 = 27;

const NX_ALPHASHIFTMASK: u32 = 0x00010000;
const NX_SHIFTMASK: u32 = 0x00020000;
const NX_CONTROLMASK: u32 = 0x00040000;
const NX_ALTERNATEMASK: u32 = 0x00080000;
const NX_COMMANDMASK: u32 = 0x00100000;

const NX_DEVICELCTLKEYMASK: u32 = 0x00000001;
const NX_DEVICELSHIFTKEYMASK: u32 = 0x00000002;
const NX_DEVICERSHIFTKEYMASK: u32 = 0x00000004;
const NX_DEVICELCMDKEYMASK: u32 = 0x00000008;
const NX_DEVICERCMDKEYMASK: u32 = 0x00000010;
const NX_DEVICELALTKEYMASK: u32 = 0x00000020;
const NX_DEVICERALTKEYMASK: u32 = 0x00000040;
const NX_DEVICERCTLKEYMASK: u32 = 0x00002000;

const K_IOHID_SET_CURSOR_POSITION: u32 = 0x00000002;
const K_IOHID_SET_RELATIVE_CURSOR_POSITION: u32 = 0x00000004;
const K_NX_EVENT_DATA_VERSION: u32 = 2;
const K_IO_HID_PARAM_CONNECT_TYPE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct IOGPoint {
    x: i16,
    y: i16,
}

#[repr(C)]
union NXEventData {
    mouse: NXEventDataMouse,
    mouse_move: NXEventDataMouseMove,
    scroll_wheel: NXEventDataScrollWheel,
    _padding: [u8; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NXEventDataMouse {
    subx: u8,
    suby: u8,
    event_num: i16,
    click: i32,
    pressure: u8,
    button_number: u8,
    sub_type: u8,
    reserved2: u8,
    reserved3: i32,
    tablet: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NXEventDataMouseMove {
    dx: i32,
    dy: i32,
    subx: u8,
    suby: u8,
    sub_type: u8,
    reserved1: u8,
    reserved2: i32,
    tablet: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NXEventDataScrollWheel {
    delta_axis1: i16,
    delta_axis2: i16,
    delta_axis3: i16,
    reserved1: i16,
    fixed_delta_axis1: i32,
    fixed_delta_axis2: i32,
    fixed_delta_axis3: i32,
    point_delta_axis1: i32,
    point_delta_axis2: i32,
    point_delta_axis3: i32,
    reserved8: [i32; 4],
}

struct IoKitHid {
    connect: u32,
}

impl IoKitHid {
    fn open() -> Option<Self> {
        unsafe {
            let service = IOServiceGetMatchingService(0, IOServiceMatching("IOHIDSystem\0".as_ptr() as _));
            if service == 0 {
                return None;
            }
            let mut connect: u32 = 0;
            let kr = IOServiceOpen(service, mach_task_self(), K_IO_HID_PARAM_CONNECT_TYPE, &mut connect);
            IOObjectRelease(service);
            if kr != 0 || connect == 0 {
                return None;
            }
            Some(Self { connect })
        }
    }

    fn post_mouse(&self, event_type: u32, cursor: CGPoint, options: u32) {
        unsafe {
            let data: NXEventData = std::mem::zeroed();
            let point = IOGPoint {
                x: cursor.x as i16,
                y: cursor.y as i16,
            };
            IOHIDPostEvent(self.connect, event_type, point, &data, K_NX_EVENT_DATA_VERSION, 0, options);
        }
    }

    fn post_mouse_relative(&self, event_type: u32, dx: i32, dy: i32) {
        unsafe {
            let mut data: NXEventData = std::mem::zeroed();
            data.mouse_move = NXEventDataMouseMove {
                dx,
                dy,
                subx: 0,
                suby: 0,
                sub_type: 0,
                reserved1: 0,
                reserved2: 0,
                tablet: [0; 32],
            };
            let point = IOGPoint { x: 0, y: 0 };
            IOHIDPostEvent(
                self.connect,
                event_type,
                point,
                &data,
                K_NX_EVENT_DATA_VERSION,
                0,
                K_IOHID_SET_RELATIVE_CURSOR_POSITION,
            );
        }
    }

    fn post_mouse_button(&self, event_type: u32, cursor: CGPoint, button: u8) {
        unsafe {
            let mut data: NXEventData = std::mem::zeroed();
            data.mouse = NXEventDataMouse {
                subx: 0,
                suby: 0,
                event_num: 0,
                click: 1,
                pressure: 0,
                button_number: button,
                sub_type: 0,
                reserved2: 0,
                reserved3: 0,
                tablet: [0; 32],
            };
            let point = IOGPoint {
                x: cursor.x as i16,
                y: cursor.y as i16,
            };
            IOHIDPostEvent(self.connect, event_type, point, &data, K_NX_EVENT_DATA_VERSION, 0, 0);
        }
    }

}

fn modifier_flag_for_keycode(keycode: u16) -> u32 {
    match keycode {
        k if k == KeyCode::SHIFT => NX_SHIFTMASK | NX_DEVICELSHIFTKEYMASK,
        k if k == KeyCode::RIGHT_SHIFT => NX_SHIFTMASK | NX_DEVICERSHIFTKEYMASK,
        k if k == KeyCode::CONTROL => NX_CONTROLMASK | NX_DEVICELCTLKEYMASK,
        k if k == KeyCode::RIGHT_CONTROL => NX_CONTROLMASK | NX_DEVICERCTLKEYMASK,
        k if k == KeyCode::OPTION => NX_ALTERNATEMASK | NX_DEVICELALTKEYMASK,
        k if k == KeyCode::RIGHT_OPTION => NX_ALTERNATEMASK | NX_DEVICERALTKEYMASK,
        k if k == KeyCode::COMMAND => NX_COMMANDMASK | NX_DEVICELCMDKEYMASK,
        k if k == KeyCode::RIGHT_COMMAND => NX_COMMANDMASK | NX_DEVICERCMDKEYMASK,
        k if k == KeyCode::CAPS_LOCK => NX_ALPHASHIFTMASK,
        _ => 0,
    }
}

/* ------------------------------------------------------------------ */
/* Helpers                                                            */
/* ------------------------------------------------------------------ */

fn current_drag_type(pressed_buttons: &HashSet<u8>) -> Option<(u32, u8)> {
    if pressed_buttons.contains(&1) {
        Some((NX_LMOUSEDRAGGED, 0))
    } else if pressed_buttons.contains(&3) {
        Some((NX_RMOUSEDRAGGED, 1))
    } else if pressed_buttons.contains(&2) {
        Some((NX_OMOUSEDRAGGED, 2))
    } else if let Some(&btn) = pressed_buttons.iter().next() {
        Some((NX_OMOUSEDRAGGED, btn))
    } else {
        None
    }
}

fn post_mouse_move(hid: &IoKitHid, cursor: CGPoint, pressed_buttons: &HashSet<u8>) {
    if let Some((event_type, _button)) = current_drag_type(pressed_buttons) {
        hid.post_mouse(event_type, cursor, K_IOHID_SET_CURSOR_POSITION);
    } else {
        hid.post_mouse(NX_MOUSEMOVED, cursor, K_IOHID_SET_CURSOR_POSITION);
    }
    let _ = CGDisplay::warp_mouse_cursor_position(cursor);
}

fn post_mouse_relative(hid: &IoKitHid, dx: i32, dy: i32, pressed_buttons: &HashSet<u8>) {
    if let Some((event_type, _button)) = current_drag_type(pressed_buttons) {
        hid.post_mouse_relative(event_type, dx, dy);
    } else {
        hid.post_mouse_relative(NX_MOUSEMOVED, dx, dy);
    }
}

fn post_mouse_button(hid: &IoKitHid, cursor: CGPoint, wire: u16, pressed: bool) {
    let wire = wire as u8;
    let (event_type, button) = match (wire, pressed) {
        (1, true) => (NX_LMOUSEDOWN, 0),
        (1, false) => (NX_LMOUSEUP, 0),
        (3, true) => (NX_RMOUSEDOWN, 1),
        (3, false) => (NX_RMOUSEUP, 1),
        (2, true) => (NX_OMOUSEDOWN, 2),
        (2, false) => (NX_OMOUSEUP, 2),
        _ => {
            let event_type = if pressed { NX_OMOUSEDOWN } else { NX_OMOUSEUP };
            (event_type, wire)
        }
    };
    hid.post_mouse_button(event_type, cursor, button);
}

fn post_scroll(_hid: &IoKitHid, source: &CGEventSource, dx: i8, dy: i8) {
    // Scroll via CGEventPost — games don't need raw scroll and the CG
    // path handles scroll acceleration / smooth-scrolling correctly.
    if let Ok(event) = CGEvent::new(source.clone()) {
        event.set_type(CGEventType::ScrollWheel);
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, i64::from(dy));
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2, i64::from(dx));
        event.post(CGEventTapLocation::HID);
    }
}
