use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMsg {
    AuthChallenge { nonce: [u8; 32], salt: String },
    AuthResponse { hmac: [u8; 32] },
    AuthResult { ok: bool },
    SessionInit { conn_id: u64, uuid: [u8; 16], encrypt: bool, auth: bool, key_timeout_ms: u16, screen_width: u16, screen_height: u16 },
    SetCaptureMode { window_mode: bool },
    Keepalive,
}
