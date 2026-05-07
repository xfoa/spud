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
    keys: HashMap<String, Instant>,
    mouse_buttons: HashMap<u8, Instant>,
    timeout: Duration,
}

impl KeyTracker {
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
            Event::KeyDown(name) => {
                let mut actions = Vec::new();
                if self.keys.contains_key(name) {
                    actions.push(format!("release {name} (lost up)"));
                }
                actions.push(format!("press {name}"));
                self.keys.insert(name.clone(), Instant::now());
                actions
            }
            Event::KeyRepeat(name) => {
                if self.keys.contains_key(name) {
                    self.keys.insert(name.clone(), Instant::now());
                    vec![format!("repeat {name}")]
                } else {
                    self.keys.insert(name.clone(), Instant::now());
                    vec![format!("press {name} (repeat without prior down)")]
                }
            }
            Event::KeyUp(name) => {
                if self.keys.remove(name).is_some() {
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
        self.keys.retain(|name, last| {
            if now.duration_since(*last) > self.timeout {
                expired.push(format!("release {name} (timeout)"));
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
}

impl SessionState {
    pub fn new(encrypt: bool, keys: Option<SessionKeys>, src_addr: SocketAddr, key_timeout_ms: u16) -> Self {
        Self {
            keys,
            replay_window: ReplayWindow::new(),
            last_activity: Instant::now(),
            src_addr,
            encrypt,
            failed_decrypts: 0,
            tracker: KeyTracker::new(key_timeout_ms),
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
