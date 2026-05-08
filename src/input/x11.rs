use std::error::Error;
use std::thread;
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::stream::Stream;
use x11rb::connection::Connection;
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::{
    ConnectionExt, EventMask, GrabMode, Keycode, ModMask,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use super::InputEvent;

const MOD_SHIFT: u16 = 1 << 0;
const MOD_LOCK: u16 = 1 << 1;
const MOD_CONTROL: u16 = 1 << 2;
const MOD_M1: u16 = 1 << 3;
const MOD_M2: u16 = 1 << 4;
const MOD_M4: u16 = 1 << 6;
const RELEVANT_MODS: u16 = MOD_SHIFT | MOD_CONTROL | MOD_M1 | MOD_M4;

pub fn listen(hotkey: String) -> impl Stream<Item = InputEvent> + Send + 'static {
    iced::stream::channel(256, move |output| async move {
        let hotkey = hotkey.clone();
        thread::spawn(move || {
            if let Err(e) = run(&hotkey, output) {
                eprintln!("[spud] X11 input backend stopped: {e}");
            }
        });
        std::future::pending::<()>().await;
    })
}

struct Pointer {
    center_x: i16,
    center_y: i16,
    edge_x: i16,
    edge_y: i16,
    last_x: i16,
    last_y: i16,
    pending_warps: u32,
}

impl Pointer {
    fn new(width: u16, height: u16) -> Self {
        let center_x = (width / 2) as i16;
        let center_y = (height / 2) as i16;
        Self {
            center_x,
            center_y,
            edge_x: (width / 4) as i16,
            edge_y: (height / 4) as i16,
            last_x: center_x,
            last_y: center_y,
            pending_warps: 0,
        }
    }
}

fn run(hotkey: &str, mut output: mpsc::Sender<InputEvent>) -> Result<(), Box<dyn Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let mut pointer = Pointer::new(screen.width_in_pixels, screen.height_in_pixels);

    conn.xfixes_query_version(5, 0)?.reply()?;

    let (modifiers, keycode) = match parse_hotkey(&conn, hotkey) {
        Ok(v) => v,
        Err(e) => {
            let _ = output.try_send(InputEvent::BackendError(format!(
                "could not parse hotkey '{hotkey}': {e}"
            )));
            return Ok(());
        }
    };

    for extra in [0, MOD_LOCK, MOD_M2, MOD_LOCK | MOD_M2] {
        conn.grab_key(
            true,
            root,
            ModMask::from(modifiers | extra),
            keycode,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?
        .check()?;
    }
    conn.flush()?;

    let mut grabbed = false;

    loop {
        if output.is_closed() {
            break;
        }

        match conn.poll_for_event()? {
            None => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Some(event) => match event {
                Event::KeyPress(kp) => {
                    let mods = u16::from(kp.state) & RELEVANT_MODS;
                    if kp.detail == keycode && mods == modifiers {
                        grabbed = !grabbed;
                        if grabbed {
                            grab_input(&conn, root, &mut pointer)?;
                        } else {
                            ungrab_input(&conn, root)?;
                        }
                        if output
                            .try_send(InputEvent::HotkeyToggled { grabbed })
                            .is_err()
                        {
                            break;
                        }
                    } else if grabbed
                        && output
                            .try_send(InputEvent::KeyPress { keycode: kp.detail })
                            .is_err()
                    {
                        break;
                    }
                }
                Event::KeyRelease(kr) if grabbed => {
                    if output
                        .try_send(InputEvent::KeyRelease { keycode: kr.detail })
                        .is_err()
                    {
                        break;
                    }
                }
                Event::MotionNotify(mn) if grabbed => {
                    let is_warp = pointer.pending_warps > 0
                        && mn.event_x == pointer.center_x
                        && mn.event_y == pointer.center_y;
                    if is_warp {
                        pointer.pending_warps -= 1;
                        pointer.last_x = pointer.center_x;
                        pointer.last_y = pointer.center_y;
                    } else {
                        let dx = mn.event_x - pointer.last_x;
                        let dy = mn.event_y - pointer.last_y;
                        pointer.last_x = mn.event_x;
                        pointer.last_y = mn.event_y;
                        if dx != 0 || dy != 0 {
                            if output
                                .try_send(InputEvent::MouseMove { dx, dy })
                                .is_err()
                            {
                                break;
                            }
                        }
                        let dist_x = (mn.event_x - pointer.center_x).abs();
                        let dist_y = (mn.event_y - pointer.center_y).abs();
                        if dist_x > pointer.edge_x || dist_y > pointer.edge_y {
                            warp_to_center(&conn, root, &mut pointer)?;
                        }
                    }
                }
                Event::ButtonPress(bp) if grabbed => {
                    let event = match bp.detail {
                        4 => Some(InputEvent::Wheel { dx: 0, dy: -1 }),
                        5 => Some(InputEvent::Wheel { dx: 0, dy: 1 }),
                        6 => Some(InputEvent::Wheel { dx: -1, dy: 0 }),
                        7 => Some(InputEvent::Wheel { dx: 1, dy: 0 }),
                        b => Some(InputEvent::MouseButton {
                            button: b,
                            pressed: true,
                        }),
                    };
                    if let Some(ev) = event {
                        if output.try_send(ev).is_err() {
                            break;
                        }
                    }
                }
                Event::ButtonRelease(br) if grabbed => {
                    // Scroll wheel buttons (4-7) are handled as Wheel events
                    // on press; ignore their release.
                    if br.detail >= 4 && br.detail <= 7 {
                        continue;
                    }
                    if output
                        .try_send(InputEvent::MouseButton {
                            button: br.detail,
                            pressed: false,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                _ => {}
            },
        }
    }

    if grabbed {
        let _ = ungrab_input(&conn, root);
    }
    for extra in [0, MOD_LOCK, MOD_M2, MOD_LOCK | MOD_M2] {
        let _ = conn.ungrab_key(keycode, root, ModMask::from(modifiers | extra));
    }
    let _ = conn.flush();
    Ok(())
}

fn grab_input(
    conn: &RustConnection,
    root: u32,
    pointer: &mut Pointer,
) -> Result<(), Box<dyn Error>> {
    conn.grab_keyboard(true, root, x11rb::CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)?
        .reply()?;
    conn.grab_pointer(
        true,
        root,
        EventMask::POINTER_MOTION | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        root,
        0u32,
        x11rb::CURRENT_TIME,
    )?
    .reply()?;
    conn.xfixes_hide_cursor(root)?;
    warp_to_center(conn, root, pointer)?;
    Ok(())
}

fn ungrab_input(conn: &RustConnection, root: u32) -> Result<(), Box<dyn Error>> {
    conn.xfixes_show_cursor(root)?;
    conn.ungrab_keyboard(x11rb::CURRENT_TIME)?;
    conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
    conn.flush()?;
    Ok(())
}

fn warp_to_center(
    conn: &RustConnection,
    root: u32,
    pointer: &mut Pointer,
) -> Result<(), Box<dyn Error>> {
    conn.warp_pointer(0u32, root, 0, 0, 0, 0, pointer.center_x, pointer.center_y)?;
    conn.flush()?;
    pointer.pending_warps += 1;
    Ok(())
}

fn parse_hotkey(
    conn: &RustConnection,
    hotkey: &str,
) -> Result<(u16, Keycode), Box<dyn Error>> {
    let mut modifiers: u16 = 0;
    let mut key_label: Option<&str> = None;

    for part in hotkey.split('+').map(|p| p.trim()) {
        match part {
            "Ctrl" => modifiers |= MOD_CONTROL,
            "Alt" => modifiers |= MOD_M1,
            "Shift" => modifiers |= MOD_SHIFT,
            "Super" | "Meta" => modifiers |= MOD_M4,
            other => {
                if key_label.is_some() {
                    return Err(format!("multiple non-modifier keys in '{hotkey}'").into());
                }
                key_label = Some(other);
            }
        }
    }

    let label = key_label.ok_or("no non-modifier key in hotkey")?;
    let keysym = label_to_keysym(label)
        .ok_or_else(|| format!("unsupported key '{label}'"))?;
    let keycode = keysym_to_keycode(conn, keysym)?
        .ok_or_else(|| format!("no keycode found for keysym {keysym:#x}"))?;

    Ok((modifiers, keycode))
}

fn keysym_to_keycode(
    conn: &RustConnection,
    keysym: u32,
) -> Result<Option<Keycode>, Box<dyn Error>> {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let count = max - min + 1;
    let mapping = conn.get_keyboard_mapping(min, count)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;

    for (i, chunk) in mapping.keysyms.chunks(per).enumerate() {
        if chunk.iter().any(|&k| k == keysym) {
            return Ok(Some(min + i as u8));
        }
    }
    Ok(None)
}

fn label_to_keysym(label: &str) -> Option<u32> {
    let s = match label {
        "Space" => return Some(0x0020),
        "Enter" => return Some(0xff0d),
        "Tab" => return Some(0xff09),
        "Backspace" => return Some(0xff08),
        "Delete" => return Some(0xffff),
        "Insert" => return Some(0xff63),
        "Home" => return Some(0xff50),
        "End" => return Some(0xff57),
        "Page Up" => return Some(0xff55),
        "Page Down" => return Some(0xff56),
        "Left" => return Some(0xff51),
        "Right" => return Some(0xff53),
        "Up" => return Some(0xff52),
        "Down" => return Some(0xff54),
        "Print Screen" => return Some(0xff61),
        "Scroll Lock" => return Some(0xff14),
        "Pause" => return Some(0xff13),
        "Caps Lock" => return Some(0xffe5),
        "Num Lock" => return Some(0xff7f),
        "F1" => return Some(0xffbe),
        "F2" => return Some(0xffbf),
        "F3" => return Some(0xffc0),
        "F4" => return Some(0xffc1),
        "F5" => return Some(0xffc2),
        "F6" => return Some(0xffc3),
        "F7" => return Some(0xffc4),
        "F8" => return Some(0xffc5),
        "F9" => return Some(0xffc6),
        "F10" => return Some(0xffc7),
        "F11" => return Some(0xffc8),
        "F12" => return Some(0xffc9),
        other => other,
    };
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if c.is_ascii_alphabetic() {
        Some(c.to_ascii_lowercase() as u32)
    } else if c.is_ascii() {
        Some(c as u32)
    } else {
        None
    }
}
