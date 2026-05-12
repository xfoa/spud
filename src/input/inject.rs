use std::io;
use std::sync::mpsc::{self, Sender as MpscSender};
use std::thread::{self, JoinHandle};

/// Commands sent to the injector worker thread.
enum InjectCmd {
    MouseAbs { x: i32, y: i32 },
    MouseRel { dx: i32, dy: i32 },
}

/// Injects mouse events into the host via Linux uinput using `kinput`.
///
/// The actual `kinput::InputDevice` lives on a dedicated thread because it
/// uses `std::rc::Rc` internally and is not `Send`.
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
            let device = kinput::InputDevice::from((
                i32::from(screen_width),
                i32::from(screen_height),
                kinput::Layout::Us,
            ));
            println!("[spud] kinput device created ({}x{})", screen_width, screen_height);

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    InjectCmd::MouseAbs { x, y } => {
                        device.mouse.abs.move_xy(x, y);
                    }
                    InjectCmd::MouseRel { dx, dy } => {
                        device.mouse.rel.move_xy(dx, dy);
                    }
                }
            }
            println!("[spud] kinput injector thread exiting");
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

    /// Legacy action parser used by the key tracker for timeout releases.
    ///
    /// Currently a no-op while keyboard / button injection is being rewritten.
    pub fn inject_action(&mut self, _action: &str) {
        // TODO: re-wire key and button injection once keyboard support is restored.
    }
}
