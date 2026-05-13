#[cfg(target_os = "linux")]
mod inject;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "linux")]
pub use inject::InputInjector;
#[cfg(target_os = "linux")]
pub use inject::{parse_key_name, wire_to_linux_button};

use iced::futures::stream::BoxStream;
#[cfg(not(target_os = "linux"))]
use iced::futures::stream;

#[derive(Debug, Clone)]
pub enum InputEvent {
    HotkeyToggled { grabbed: bool },
    KeyPress { keycode: u8 },
    KeyRelease { keycode: u8 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, pressed: bool },
    Wheel { dx: i8, dy: i8 },
    // Can remove linter allow when input for other platforms is implemented
    #[allow(unused)]
    BackendError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaylandHandles {
    pub display: usize,
    pub surface: usize,
}

pub fn extract_wayland_handles(window: &dyn iced::Window) -> Option<WaylandHandles> {
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

    let display = match window.display_handle().ok()?.as_raw() {
        RawDisplayHandle::Wayland(d) => d.display.as_ptr() as usize,
        _ => return None,
    };
    let surface = match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Wayland(w) => w.surface.as_ptr() as usize,
        _ => return None,
    };
    Some(WaylandHandles { display, surface })
}

pub fn listen(hotkey: String) -> BoxStream<'static, InputEvent> {
    #[cfg(target_os = "linux")]
    {
        return Box::pin(x11::listen(hotkey));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = hotkey;
        Box::pin(stream::once(async {
            InputEvent::BackendError(
                "hotkey mode is not yet implemented for this platform".to_string(),
            )
        }))
    }
}

pub fn listen_wayland(handles: WaylandHandles) -> BoxStream<'static, InputEvent> {
    #[cfg(target_os = "linux")]
    {
        return Box::pin(wayland::listen(handles));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = handles;
        Box::pin(stream::once(async {
            InputEvent::BackendError(
                "wayland hotkey mode is only available on Linux".to_string(),
            )
        }))
    }
}

#[cfg(target_os = "linux")]
pub fn toggle_wayland_grab() -> bool {
    wayland::signal().toggle()
}

#[cfg(not(target_os = "linux"))]
pub fn toggle_wayland_grab() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub fn is_wayland_grabbed() -> bool {
    wayland::signal().is_grabbed()
}

#[cfg(not(target_os = "linux"))]
pub fn is_wayland_grabbed() -> bool {
    false
}
