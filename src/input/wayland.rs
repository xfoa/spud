use std::error::Error;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use iced::futures::channel::mpsc;
use iced::futures::stream::Stream;
use iced::futures::StreamExt;
use wayland_backend::sys::client::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_pointer, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_locked_pointer_v1, zwp_pointer_constraints_v1,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1, zwp_relative_pointer_v1,
};

use super::{InputEvent, WaylandHandles};

const SHORTCUT_ID: &str = "spud-toggle-grab";

pub fn listen(
    hotkey: String,
    handles: WaylandHandles,
) -> impl Stream<Item = InputEvent> + Send + 'static {
    iced::stream::channel(256, move |output: mpsc::Sender<InputEvent>| async move {
        let signal = Arc::new(GrabSignal::default());
        let portal_signal = signal.clone();
        let portal_output = output.clone();
        let portal_hotkey = hotkey.clone();
        thread::spawn(move || {
            run_portal(portal_hotkey, portal_signal, portal_output);
        });
        thread::spawn(move || {
            if let Err(e) = run_wayland(handles, signal, output) {
                eprintln!("[spud] Wayland input backend stopped: {e}");
            }
        });
        std::future::pending::<()>().await;
    })
}

#[derive(Default)]
struct GrabSignal {
    grabbed: AtomicBool,
    dirty: AtomicBool,
}

impl GrabSignal {
    fn toggle(&self) -> bool {
        let prev = self.grabbed.fetch_xor(true, Ordering::SeqCst);
        self.dirty.store(true, Ordering::SeqCst);
        !prev
    }
    fn take_dirty(&self) -> Option<bool> {
        if self.dirty.swap(false, Ordering::SeqCst) {
            Some(self.grabbed.load(Ordering::SeqCst))
        } else {
            None
        }
    }
}

fn run_portal(hotkey: String, signal: Arc<GrabSignal>, output: mpsc::Sender<InputEvent>) {
    if let Err(e) = async_io::block_on(portal_loop(hotkey, signal, output)) {
        eprintln!("[spud] GlobalShortcuts portal stopped: {e}");
    }
}

async fn portal_loop(
    hotkey: String,
    signal: Arc<GrabSignal>,
    mut output: mpsc::Sender<InputEvent>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let proxy = GlobalShortcuts::new().await?;
    let session = proxy.create_session(Default::default()).await?;
    let trigger = portal_trigger_from_chord(&hotkey);
    let shortcut =
        NewShortcut::new(SHORTCUT_ID, "Toggle remote input capture").preferred_trigger(trigger.as_deref());

    proxy
        .bind_shortcuts(&session, &[shortcut], None, Default::default())
        .await?
        .response()?;

    let mut activated = proxy.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if event.shortcut_id() != SHORTCUT_ID {
            continue;
        }
        if output.is_closed() {
            break;
        }
        let grabbed = signal.toggle();
        if output
            .try_send(InputEvent::HotkeyToggled { grabbed })
            .is_err()
        {
            break;
        }
    }
    Ok(())
}

fn portal_trigger_from_chord(hotkey: &str) -> Option<String> {
    let mut out = String::new();
    let mut key: Option<&str> = None;
    for part in hotkey.split('+').map(str::trim) {
        match part {
            "Ctrl" => out.push_str("<Ctrl>"),
            "Alt" => out.push_str("<Alt>"),
            "Shift" => out.push_str("<Shift>"),
            "Super" | "Meta" => out.push_str("<Super>"),
            other => {
                if key.is_some() {
                    return None;
                }
                key = Some(other);
            }
        }
    }
    let label = key?;
    let key_str = match label {
        "Space" => "space",
        "Enter" => "Return",
        "Tab" => "Tab",
        "Backspace" => "BackSpace",
        "Delete" => "Delete",
        "Insert" => "Insert",
        "Home" => "Home",
        "End" => "End",
        "Page Up" => "Page_Up",
        "Page Down" => "Page_Down",
        "Left" => "Left",
        "Right" => "Right",
        "Up" => "Up",
        "Down" => "Down",
        "Print Screen" => "Print",
        "Scroll Lock" => "Scroll_Lock",
        "Pause" => "Pause",
        "Caps Lock" => "Caps_Lock",
        "Num Lock" => "Num_Lock",
        s if s.starts_with('F') => s,
        other => {
            let mut chars = other.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if c.is_ascii_alphabetic() {
                    out.push(c.to_ascii_lowercase());
                    return Some(out);
                }
                out.push(c);
                return Some(out);
            }
            return None;
        }
    };
    out.push_str(key_str);
    Some(out)
}

struct State {
    output: mpsc::Sender<InputEvent>,
    pending_dx: f64,
    pending_dy: f64,
    grabbed: bool,
    pending_axis_x: f64,
    pending_axis_y: f64,
}

fn run_wayland(
    handles: WaylandHandles,
    signal: Arc<GrabSignal>,
    mut output: mpsc::Sender<InputEvent>,
) -> Result<(), Box<dyn Error>> {
    let backend = unsafe { Backend::from_foreign_display(handles.display as *mut _) };
    let conn = Connection::from_backend(backend);

    let surface_id = unsafe {
        ObjectId::from_ptr(
            wl_surface::WlSurface::interface(),
            handles.surface as *mut _,
        )?
    };
    let surface = wl_surface::WlSurface::from_id(&conn, surface_id)?;

    let (globals, mut event_queue) = registry_queue_init::<State>(&conn)?;
    let qh = event_queue.handle();

    let seat: wl_seat::WlSeat = match globals.bind(&qh, 1..=8, ()) {
        Ok(s) => s,
        Err(e) => {
            let _ = output.try_send(InputEvent::BackendError(format!("wl_seat: {e}")));
            return Ok(());
        }
    };
    let constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 =
        match globals.bind(&qh, 1..=1, ()) {
            Ok(c) => c,
            Err(e) => {
                let _ = output.try_send(InputEvent::BackendError(format!(
                    "zwp_pointer_constraints_v1: {e}"
                )));
                return Ok(());
            }
        };
    let rel_manager: zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1 =
        match globals.bind(&qh, 1..=1, ()) {
            Ok(r) => r,
            Err(e) => {
                let _ = output.try_send(InputEvent::BackendError(format!(
                    "zwp_relative_pointer_manager_v1: {e}"
                )));
                return Ok(());
            }
        };

    let pointer = seat.get_pointer(&qh, ());
    let _relative = rel_manager.get_relative_pointer(&pointer, &qh, ());

    let mut state = State {
        output: output.clone(),
        pending_dx: 0.0,
        pending_dy: 0.0,
        grabbed: false,
        pending_axis_x: 0.0,
        pending_axis_y: 0.0,
    };

    let mut locked: Option<zwp_locked_pointer_v1::ZwpLockedPointerV1> = None;

    conn.flush()?;

    loop {
        if state.output.is_closed() {
            break;
        }

        if let Some(grabbed) = signal.take_dirty() {
            state.grabbed = grabbed;
            if grabbed {
                if locked.is_none() {
                    let lock = constraints.lock_pointer(
                        &surface,
                        &pointer,
                        None,
                        zwp_pointer_constraints_v1::Lifetime::Persistent,
                        &qh,
                        (),
                    );
                    locked = Some(lock);
                }
            } else if let Some(l) = locked.take() {
                l.destroy();
            }
            conn.flush()?;
        }

        event_queue.dispatch_pending(&mut state)?;
        conn.flush()?;

        let read_guard = match event_queue.prepare_read() {
            Some(g) => g,
            None => continue,
        };

        let fd = read_guard.connection_fd().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut pfd, 1, 50) };

        if result > 0 && (pfd.revents & libc::POLLIN) != 0 {
            read_guard.read()?;
        } else {
            drop(read_guard);
        }
    }

    if let Some(l) = locked.take() {
        l.destroy();
        let _ = conn.flush();
    }

    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: <wl_registry::WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if !state.grabbed {
            return;
        }
        match event {
            wl_pointer::Event::Button {
                button,
                state: btn_state,
                ..
            } => {
                let pressed = matches!(
                    btn_state,
                    WEnum::Value(wl_pointer::ButtonState::Pressed)
                );
                let _ = state.output.try_send(InputEvent::MouseButton {
                    button: map_button(button),
                    pressed,
                });
            }
            wl_pointer::Event::Axis { axis, value, .. } => match axis {
                WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                    state.pending_axis_y += value;
                    emit_axis_buttons(state, false);
                }
                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                    state.pending_axis_x += value;
                    emit_axis_buttons(state, true);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Dispatch<zwp_relative_pointer_v1::ZwpRelativePointerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &zwp_relative_pointer_v1::ZwpRelativePointerV1,
        event: <zwp_relative_pointer_v1::ZwpRelativePointerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if !state.grabbed {
            return;
        }
        if let zwp_relative_pointer_v1::Event::RelativeMotion { dx, dy, .. } = event {
            state.pending_dx += dx;
            state.pending_dy += dy;
            let idx = state.pending_dx.trunc() as i16;
            let idy = state.pending_dy.trunc() as i16;
            state.pending_dx -= idx as f64;
            state.pending_dy -= idy as f64;
            if idx != 0 || idy != 0 {
                let _ = state.output.try_send(InputEvent::MouseMove {
                    dx: idx,
                    dy: idy,
                });
            }
        }
    }
}

fn emit_axis_buttons(state: &mut State, horizontal: bool) {
    const STEP: f64 = 10.0;
    let value_ref = if horizontal {
        &mut state.pending_axis_x
    } else {
        &mut state.pending_axis_y
    };
    while value_ref.abs() >= STEP {
        let positive = *value_ref > 0.0;
        *value_ref -= if positive { STEP } else { -STEP };
        let button = match (horizontal, positive) {
            (false, false) => 4, // wheel up
            (false, true) => 5,  // wheel down
            (true, false) => 6,  // wheel left
            (true, true) => 7,   // wheel right
        };
        let _ = state.output.try_send(InputEvent::MouseButton {
            button,
            pressed: true,
        });
        let _ = state.output.try_send(InputEvent::MouseButton {
            button,
            pressed: false,
        });
    }
}

fn map_button(button: u32) -> u8 {
    match button {
        0x110 => 1,
        0x111 => 3,
        0x112 => 2,
        0x113 => 8,
        0x114 => 9,
        other => (other & 0xff) as u8,
    }
}

wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(State: ignore zwp_pointer_constraints_v1::ZwpPointerConstraintsV1);
wayland_client::delegate_noop!(State: ignore zwp_locked_pointer_v1::ZwpLockedPointerV1);
wayland_client::delegate_noop!(State: ignore zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1);
