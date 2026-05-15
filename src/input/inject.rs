use std::io;
use std::io::Write;
use std::sync::mpsc::{self, Sender as MpscSender};
use std::thread::{self, JoinHandle};

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

/// Injects input events into the host via Linux uinput.
///
/// Mouse movement uses `kinput` (already working well).
/// Keyboard, mouse buttons, and wheel use `evdev::VirtualDevice`.
pub struct InputInjector {
    tx: MpscSender<InjectCmd>,
    _handle: JoinHandle<()>,
    pub helper: Option<std::process::Child>,
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

        Ok(Self { tx, _handle: handle, helper: None })
    }

    /// Create an injector that forwards events over a Unix socket to a
    /// privileged helper process (e.g. started via pkexec).
    pub fn new_ipc(socket_path: &str) -> io::Result<Self> {
        let stream = std::os::unix::net::UnixStream::connect(socket_path)?;
        let (tx, rx) = mpsc::channel::<InjectCmd>();
        let mut stream = stream;
        let handle = thread::spawn(move || {
            while let Ok(cmd) = rx.recv() {
                let bytes = match postcard::to_allocvec(&cmd) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[spud] IPC serialize error: {e}");
                        break;
                    }
                };
                let len = match u16::try_from(bytes.len()) {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("[spud] IPC command too large");
                        break;
                    }
                };
                if let Err(e) = stream.write_all(&len.to_le_bytes()) {
                    eprintln!("[spud] IPC write error: {e}");
                    break;
                }
                if let Err(e) = stream.write_all(&bytes) {
                    eprintln!("[spud] IPC write error: {e}");
                    break;
                }
            }
            println!("[spud] IPC input injector thread exiting");
        });
        Ok(Self { tx, _handle: handle, helper: None })
    }

    /// Move the cursor to an absolute screen position (pixels).
    pub fn move_abs(&self, x: i32, y: i32) {
        if let Err(e) = self.tx.send(InjectCmd::MouseAbs { x, y }) {
            eprintln!("[spud] Inject move_abs failed: channel closed ({e})");
        }
    }

    /// Move the cursor by a relative delta (pixels).
    pub fn move_rel(&self, dx: i32, dy: i32) {
        if let Err(e) = self.tx.send(InjectCmd::MouseRel { dx, dy }) {
            eprintln!("[spud] Inject move_rel failed: channel closed ({e})");
        }
    }

    /// Press a keyboard key by Linux evdev keycode.
    pub fn key_down(&self, code: u16) {
        if let Err(e) = self.tx.send(InjectCmd::KeyDown { code }) {
            eprintln!("[spud] Inject key_down failed: channel closed ({e})");
        }
    }

    /// Release a keyboard key by Linux evdev keycode.
    pub fn key_up(&self, code: u16) {
        if let Err(e) = self.tx.send(InjectCmd::KeyUp { code }) {
            eprintln!("[spud] Inject key_up failed: channel closed ({e})");
        }
    }

    /// Press a mouse button by Linux evdev button code.
    pub fn button_down(&self, code: u16) {
        if let Err(e) = self.tx.send(InjectCmd::ButtonDown { code }) {
            eprintln!("[spud] Inject button_down failed: channel closed ({e})");
        }
    }

    /// Release a mouse button by Linux evdev button code.
    pub fn button_up(&self, code: u16) {
        if let Err(e) = self.tx.send(InjectCmd::ButtonUp { code }) {
            eprintln!("[spud] Inject button_up failed: channel closed ({e})");
        }
    }

    /// Emit a mouse wheel event.
    pub fn wheel(&self, dx: i8, dy: i8) {
        if let Err(e) = self.tx.send(InjectCmd::Wheel { dx, dy }) {
            eprintln!("[spud] Inject wheel failed: channel closed ({e})");
        }
    }

    /// Legacy action parser used by the key tracker for timeout releases.
    pub fn inject_action(&self, action: &str) {
        let action = action.trim();
        if let Some(rest) = action.strip_prefix("press ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = crate::input::key_names::parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyDown { code });
            } else if let Some(btn) = crate::input::key_names::parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonDown { code: crate::input::wire_to_linux_button(btn) });
            }
        } else if let Some(rest) = action.strip_prefix("release ") {
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            if let Some(code) = crate::input::key_names::parse_key_name(name) {
                let _ = self.tx.send(InjectCmd::KeyUp { code });
            } else if let Some(btn) = crate::input::key_names::parse_mouse_button(name) {
                let _ = self.tx.send(InjectCmd::ButtonUp { code: crate::input::wire_to_linux_button(btn) });
            }
        }
        // repeat actions are ignored for injection (they're just heartbeats)
    }
}

impl Drop for InputInjector {
    fn drop(&mut self) {
        if let Some(mut child) = self.helper.take() {
            let _ = child.kill();
        }
    }
}

pub fn create_evdev_device() -> io::Result<evdev::uinput::VirtualDevice> {
    let mut keys = evdev::AttributeSet::<evdev::KeyCode>::new();
    // Skip 0 (KEY_RESERVED) which can cause some display servers to ignore
    // the device. Start from 1 to cover all real key/button codes.
    for code in 1..=0x2ffu16 {
        keys.insert(evdev::KeyCode::new(code));
    }

    let mut rel_axes = evdev::AttributeSet::<evdev::RelativeAxisCode>::new();
    rel_axes.insert(evdev::RelativeAxisCode::REL_X);
    rel_axes.insert(evdev::RelativeAxisCode::REL_Y);
    rel_axes.insert(evdev::RelativeAxisCode::REL_WHEEL);
    rel_axes.insert(evdev::RelativeAxisCode::REL_HWHEEL);

    evdev::uinput::VirtualDevice::builder()?
        .name(b"spud virtual input")
        .with_keys(&keys)?
        .with_relative_axes(&rel_axes)?
        .build()
}

pub fn emit_key(dev: &mut evdev::uinput::VirtualDevice, code: u16, value: i32) -> io::Result<()> {
    use evdev::{EventType, InputEvent, KeyCode, SynchronizationCode};
    dev.emit(&[
        InputEvent::new_now(EventType::KEY.0, KeyCode::new(code).0, value),
        InputEvent::new_now(EventType::SYNCHRONIZATION.0, SynchronizationCode::SYN_REPORT.0, 0),
    ])
}

pub fn emit_wheel(dev: &mut evdev::uinput::VirtualDevice, dx: i8, dy: i8) -> io::Result<()> {
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
