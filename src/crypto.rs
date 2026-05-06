use aes_gcm::aead::{Aead, AeadCore, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

const WINDOW_SIZE: usize = 1024;
const WORDS: usize = WINDOW_SIZE / 64;

/// RFC 4303-style replay window.
#[derive(Debug, Clone)]
pub struct ReplayWindow {
    max_seq: u64,
    bitmap: [u64; WORDS],
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self {
            max_seq: 0,
            bitmap: [0u64; WORDS],
        }
    }

    /// Returns true if the sequence number is valid (not duplicate, not too old).
    pub fn is_valid(&mut self, seq: u64) -> bool {
        if seq == 0 {
            return false;
        }

        if seq > self.max_seq {
            let diff = (seq - self.max_seq) as usize;
            let shift = diff.min(WINDOW_SIZE);

            if shift >= WINDOW_SIZE {
                self.bitmap = [0u64; WORDS];
            } else {
                let word_shift = shift / 64;
                let bit_shift = shift % 64;

                let mut carry = 0u64;
                for i in 0..WORDS {
                    let new_carry = self.bitmap[i] >> (64 - bit_shift);
                    self.bitmap[i] = (self.bitmap[i] << bit_shift) | carry;
                    carry = new_carry;
                }

                for i in 0..word_shift.min(WORDS) {
                    self.bitmap[i] = 0;
                }
            }

            self.max_seq = seq;
            self.bitmap[0] |= 1;
            true
        } else {
            let diff = self.max_seq - seq;
            if diff >= WINDOW_SIZE as u64 {
                return false;
            }

            let word = (diff / 64) as usize;
            let bit = diff % 64;
            let mask = 1u64 << bit;

            if self.bitmap[word] & mask != 0 {
                return false;
            }

            self.bitmap[word] |= mask;
            true
        }
    }
}

/// Per-direction UDP keys derived from TLS exporter.
#[derive(Zeroize, ZeroizeOnDrop, Debug, Clone)]
pub struct UdpKeys {
    pub client_write: [u8; 32],
    pub server_write: [u8; 32],
}

/// Derive client/server UDP keys from 64 bytes of TLS exported material.
pub fn derive_udp_keys(exported: &[u8; 64]) -> UdpKeys {
    let hkdf = Hkdf::<Sha256>::new(None, exported);
    let mut client_write = [0u8; 32];
    let mut server_write = [0u8; 32];
    hkdf.expand(b"client-write", &mut client_write).unwrap();
    hkdf.expand(b"server-write", &mut server_write).unwrap();
    UdpKeys { client_write, server_write }
}

/// Encrypt an event with AES-256-GCM. Sequence number is bound as AAD.
pub fn encrypt_event(cipher: &Aes256Gcm, seq: u64, plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let seq_bytes = seq.to_le_bytes();
    let payload = Payload {
        msg: plaintext,
        aad: &seq_bytes,
    };
    let mut ciphertext = cipher.encrypt(&nonce, payload)?;
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(nonce.as_slice());
    result.append(&mut ciphertext);
    Ok(result)
}

/// Decrypt an event with AES-256-GCM. Sequence number is verified as AAD.
pub fn decrypt_event(cipher: &Aes256Gcm, seq: u64, nonce_ct: &[u8]) -> Option<Vec<u8>> {
    if nonce_ct.len() < 12 + 16 {
        return None;
    }
    let nonce = Nonce::from_slice(&nonce_ct[..12]);
    let ciphertext = &nonce_ct[12..];
    let seq_bytes = seq.to_le_bytes();
    let payload = Payload {
        msg: ciphertext,
        aad: &seq_bytes,
    };
    cipher.decrypt(nonce, payload).ok()
}
