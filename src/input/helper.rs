//! Privileged helper that opens /dev/uinput and receives commands over a
//! Unix socket. Run via pkexec, e.g.
//!   pkexec /path/to/spud injection-helper /tmp/spud-input.sock 1920 1080

use std::io;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};

use crate::input::inject::{create_evdev_device, emit_key, emit_wheel, InjectCmd};

/// Entry point for the injection-helper binary.
///
/// `socket_path` is a Unix socket the main app will connect to.
/// `screen_width` / `screen_height` are used for kinput absolute positioning.
pub fn run(socket_path: &str, screen_width: u16, screen_height: u16) -> io::Result<()> {
    // Create the uinput device and kinput before listening so any failure
    // happens early while pkexec dialog is still fresh.
    let kinput_device = kinput::InputDevice::from((
        i32::from(screen_width),
        i32::from(screen_height),
        kinput::Layout::Us,
    ));

    let mut evdev = create_evdev_device()?;

    // Remove stale socket.
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    // Make socket writable by anyone so the user process can connect.
    let perms = std::fs::Permissions::from_mode(0o777);
    std::fs::set_permissions(socket_path, perms)?;
    println!("[spud-injection-helper] listening on {socket_path}");

    let mut stream = match listener.accept() {
        Ok((s, _)) => s,
        Err(e) => {
            eprintln!("[spud-injection-helper] accept failed: {e}");
            return Err(e);
        }
    };

    println!("[spud-injection-helper] client connected");

    if let Err(e) = handle_stream(&mut stream, &kinput_device, &mut evdev) {
        eprintln!("[spud-injection-helper] connection closed: {e}");
    }

    let _ = std::fs::remove_file(socket_path);
    Ok(())
}

fn handle_stream(
    stream: &mut UnixStream,
    kinput_device: &kinput::InputDevice,
    evdev: &mut evdev::uinput::VirtualDevice,
) -> io::Result<()> {
    loop {
        let mut len_buf = [0u8; 2];
        stream.read_exact(&mut len_buf)?;
        let len = u16::from_le_bytes(len_buf) as usize;
        if len == 0 {
            continue;
        }
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf)?;
        let cmd: InjectCmd = match postcard::from_bytes(&buf) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[spud-injection-helper] bad command: {e}");
                continue;
            }
        };
        match cmd {
            InjectCmd::MouseAbs { x, y } => {
                let _ = kinput_device.mouse.abs.move_xy(x, y);
            }
            InjectCmd::MouseRel { dx, dy } => {
                let _ = kinput_device.mouse.rel.move_xy(dx, dy);
            }
            InjectCmd::KeyDown { code } => {
                if let Err(e) = emit_key(evdev, code, 1) {
                    eprintln!("[spud-injection-helper] emit_key failed: {e}");
                }
            }
            InjectCmd::KeyUp { code } => {
                if let Err(e) = emit_key(evdev, code, 0) {
                    eprintln!("[spud-injection-helper] emit_key failed: {e}");
                }
            }
            InjectCmd::ButtonDown { code } => {
                if let Err(e) = emit_key(evdev, code, 1) {
                    eprintln!("[spud-injection-helper] emit_key failed: {e}");
                }
            }
            InjectCmd::ButtonUp { code } => {
                if let Err(e) = emit_key(evdev, code, 0) {
                    eprintln!("[spud-injection-helper] emit_key failed: {e}");
                }
            }
            InjectCmd::Wheel { dx, dy } => {
                if let Err(e) = emit_wheel(evdev, dx, dy) {
                    eprintln!("[spud-injection-helper] emit_wheel failed: {e}");
                }
            }
        }
    }
}
