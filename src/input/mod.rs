#[cfg(target_os = "linux")]
mod x11;

use iced::futures::stream::{self, BoxStream};

#[derive(Debug, Clone)]
pub enum InputEvent {
    HotkeyToggled { grabbed: bool },
    KeyPress { keycode: u8 },
    KeyRelease { keycode: u8 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, pressed: bool },
    BackendError(String),
}

pub fn listen(hotkey: String) -> BoxStream<'static, InputEvent> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Box::pin(x11::listen(hotkey));
        }
        return Box::pin(stream::once(async {
            InputEvent::BackendError(
                "hotkey mode is not yet implemented for Wayland".to_string(),
            )
        }));
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
