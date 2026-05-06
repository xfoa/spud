use std::net::SocketAddr;
use std::time::Instant;

use dashmap::DashMap;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::ReplayWindow;

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

pub struct SessionState {
    pub keys: Option<SessionKeys>,
    pub replay_window: ReplayWindow,
    pub last_activity: Instant,
    pub src_addr: SocketAddr,
    pub encrypt: bool,
    pub failed_decrypts: u32,
}

impl SessionState {
    pub fn new(encrypt: bool, keys: Option<SessionKeys>, src_addr: SocketAddr) -> Self {
        Self {
            keys,
            replay_window: ReplayWindow::new(),
            last_activity: Instant::now(),
            src_addr,
            encrypt,
            failed_decrypts: 0,
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
