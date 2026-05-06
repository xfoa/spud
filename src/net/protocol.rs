use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMsg {
    AuthChallenge { nonce: [u8; 32], salt: String },
    AuthResponse { hmac: [u8; 32] },
    AuthResult { ok: bool },
    SessionInit { conn_id: u64, uuid: [u8; 16], encrypt: bool, key_timeout_ms: u16 },
    Keepalive,
}
