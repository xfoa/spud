use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::ReplayWindow;
use crate::net::Event;

/// Return a human-readable name for an evdev scancode.
fn evdev_name(code: u16) -> String {
    // Reverse of the logical mapping in input::inject::parse_key_name.
    let name = match code {
        1 => "Escape",
        2 => "Digit1",
        3 => "Digit2",
        4 => "Digit3",
        5 => "Digit4",
        6 => "Digit5",
        7 => "Digit6",
        8 => "Digit7",
        9 => "Digit8",
        10 => "Digit9",
        11 => "Digit0",
        12 => "Minus",
        13 => "Equal",
        14 => "Backspace",
        15 => "Tab",
        16 => "KeyQ",
        17 => "KeyW",
        18 => "KeyE",
        19 => "KeyR",
        20 => "KeyT",
        21 => "KeyY",
        22 => "KeyU",
        23 => "KeyI",
        24 => "KeyO",
        25 => "KeyP",
        26 => "BracketLeft",
        27 => "BracketRight",
        28 => "Enter",
        29 => "ControlLeft",
        30 => "KeyA",
        31 => "KeyS",
        32 => "KeyD",
        33 => "KeyF",
        34 => "KeyG",
        35 => "KeyH",
        36 => "KeyJ",
        37 => "KeyK",
        38 => "KeyL",
        39 => "Semicolon",
        40 => "Quote",
        41 => "Backquote",
        42 => "ShiftLeft",
        43 => "Backslash",
        44 => "KeyZ",
        45 => "KeyX",
        46 => "KeyC",
        47 => "KeyV",
        48 => "KeyB",
        49 => "KeyN",
        50 => "KeyM",
        51 => "Comma",
        52 => "Period",
        53 => "Slash",
        54 => "ShiftRight",
        55 => "NumpadMultiply",
        56 => "AltLeft",
        57 => "Space",
        58 => "CapsLock",
        59 => "F1",
        60 => "F2",
        61 => "F3",
        62 => "F4",
        63 => "F5",
        64 => "F6",
        65 => "F7",
        66 => "F8",
        67 => "F9",
        68 => "F10",
        69 => "NumLock",
        70 => "ScrollLock",
        71 => "Numpad7",
        72 => "Numpad8",
        73 => "Numpad9",
        74 => "NumpadSubtract",
        75 => "Numpad4",
        76 => "Numpad5",
        77 => "Numpad6",
        78 => "NumpadAdd",
        79 => "Numpad1",
        80 => "Numpad2",
        81 => "Numpad3",
        82 => "Numpad0",
        83 => "NumpadDecimal",
        86 => "IntlBackslash",
        87 => "F11",
        88 => "F12",
        89 => "IntlRo",
        92 => "Convert",
        93 => "KanaMode",
        94 => "NonConvert",
        96 => "NumpadEnter",
        97 => "ControlRight",
        98 => "NumpadDivide",
        99 => "PrintScreen",
        100 => "AltRight",
        102 => "Home",
        103 => "ArrowUp",
        104 => "PageUp",
        105 => "ArrowLeft",
        106 => "ArrowRight",
        107 => "End",
        108 => "ArrowDown",
        109 => "PageDown",
        110 => "Insert",
        111 => "Delete",
        119 => "Pause",
        125 => "SuperLeft",
        126 => "SuperRight",
        127 => "ContextMenu",
        138 => "Help",
        122 => "Lang1",
        123 => "Lang2",
        90 => "Lang3",
        91 => "Lang4",
        85 => "Lang5",
        121 => "NumpadComma",
        117 => "NumpadEqual",
        _ => return format!("evdev:{code}"),
    };
    name.to_string()
}

pub type SessionUuid = [u8; 16];
pub type ConnId = u64;

/// Generate a random session UUID and derive a ConnID from it.
pub fn generate_session() -> (SessionUuid, ConnId) {
    let mut uuid = [0u8; 16];
    OsRng.fill_bytes(&mut uuid);

    let hkdf = Hkdf::<Sha256>::new(None, &uuid);
    let mut conn_id_bytes = [0u8; 8];
    hkdf.expand(b"spud-conn-id", &mut conn_id_bytes).unwrap();

    let conn_id = u64::from_le_bytes(conn_id_bytes);
    (uuid, conn_id)
}

/// Session keys with secure zeroing on drop.
#[derive(Zeroize, ZeroizeOnDrop, Debug, Clone)]
pub struct SessionKeys {
    pub server_read: [u8; 32],
    pub server_write: [u8; 32],
}

/// Per-session state stored in the server's session table.
const MAX_FAILED_DECRYPTS: u32 = 10;

/// Tracks held keys and mouse buttons, releasing them on timeout.
pub struct KeyTracker {
    keys: HashMap<u16, Instant>,
    mouse_buttons: HashMap<u8, Instant>,
    timeout: Duration,
}

impl KeyTracker {
    pub fn has_key(&self, code: u16) -> bool {
        self.keys.contains_key(&code)
    }

    pub fn has_button(&self, button: u8) -> bool {
        self.mouse_buttons.contains_key(&button)
    }

    pub fn new(timeout_ms: u16) -> Self {
        Self {
            keys: HashMap::new(),
            mouse_buttons: HashMap::new(),
            timeout: Duration::from_millis(u64::from(timeout_ms)),
        }
    }

    /// Process a single event and return any actions taken.
    pub fn handle_event(&mut self, event: &Event) -> Vec<String> {
        match event {
            Event::KeyDown(code) => {
                let mut actions = Vec::new();
                let name = evdev_name(*code);
                if self.keys.contains_key(code) {
                    actions.push(format!("release {name} (lost up)"));
                }
                actions.push(format!("press {name}"));
                self.keys.insert(*code, Instant::now());
                actions
            }
            Event::KeyRepeat(code) => {
                let name = evdev_name(*code);
                if self.keys.contains_key(code) {
                    self.keys.insert(*code, Instant::now());
                    vec![format!("repeat {name}")]
                } else {
                    self.keys.insert(*code, Instant::now());
                    vec![format!("press {name} (repeat without prior down)")]
                }
            }
            Event::KeyUp(code) => {
                let name = evdev_name(*code);
                if self.keys.remove(code).is_some() {
                    vec![format!("release {name}")]
                } else {
                    Vec::new()
                }
            }
            Event::MouseButton { button, pressed: true } => {
                let mut actions = Vec::new();
                if self.mouse_buttons.contains_key(button) {
                    actions.push(format!("release mouse {button} (lost up)"));
                }
                actions.push(format!("press mouse {button}"));
                self.mouse_buttons.insert(*button, Instant::now());
                actions
            }
            Event::MouseButtonRepeat(button) => {
                if self.mouse_buttons.contains_key(button) {
                    self.mouse_buttons.insert(*button, Instant::now());
                    vec![format!("repeat mouse {button}")]
                } else {
                    self.mouse_buttons.insert(*button, Instant::now());
                    vec![format!("press mouse {button} (repeat without prior down)")]
                }
            }
            Event::MouseButton { button, pressed: false } => {
                if self.mouse_buttons.remove(button).is_some() {
                    vec![format!("release mouse {button}")]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// Release any keys/buttons that have been held longer than the timeout.
    pub fn sweep(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut expired = Vec::new();
        self.keys.retain(|code, last| {
            if now.duration_since(*last) > self.timeout {
                expired.push(format!("release {} (timeout)", evdev_name(*code)));
                false
            } else {
                true
            }
        });
        self.mouse_buttons.retain(|button, last| {
            if now.duration_since(*last) > self.timeout {
                expired.push(format!("release mouse {button} (timeout)"));
                false
            } else {
                true
            }
        });
        expired
    }
}

pub struct SessionState {
    pub keys: Option<SessionKeys>,
    pub replay_window: ReplayWindow,
    pub last_activity: Instant,
    pub src_addr: SocketAddr,
    pub encrypt: bool,
    pub failed_decrypts: u32,
    pub tracker: KeyTracker,
    pub screen_width: u16,
    pub screen_height: u16,
    pub window_mode: bool,
}

impl SessionState {
    pub fn new(encrypt: bool, keys: Option<SessionKeys>, src_addr: SocketAddr, key_timeout_ms: u16, screen_width: u16, screen_height: u16) -> Self {
        Self {
            keys,
            replay_window: ReplayWindow::new(),
            last_activity: Instant::now(),
            src_addr,
            encrypt,
            failed_decrypts: 0,
            tracker: KeyTracker::new(key_timeout_ms),
            screen_width,
            screen_height,
            window_mode: false,
        }
    }

    pub fn record_decrypt_success(&mut self) {
        self.failed_decrypts = 0;
    }

    pub fn record_decrypt_failure(&mut self) -> bool {
        self.failed_decrypts += 1;
        self.failed_decrypts >= MAX_FAILED_DECRYPTS
    }
}

pub type SessionTable = DashMap<ConnId, SessionState>;
