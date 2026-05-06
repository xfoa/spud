use hmac::{Hmac, Mac};
use rand_core::OsRng;
use rand_core::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::config::{extract_salt, hash_passphrase_with_salt};

/// Generate a 32-byte random challenge.
pub fn generate_challenge() -> [u8; 32] {
    let mut challenge = [0u8; 32];
    OsRng.fill_bytes(&mut challenge);
    challenge
}

/// Client computes HMAC-SHA256(Argon2(passphrase, salt), challenge).
pub fn client_compute_response(passphrase: &str, salt: &str, challenge: &[u8; 32]) -> Option<[u8; 32]> {
    let hash = hash_passphrase_with_salt(passphrase, salt)?;
    let parsed = argon2::password_hash::PasswordHash::new(&hash).ok()?;
    let hash_bytes = parsed.hash?.as_bytes();

    let mut mac = Hmac::<Sha256>::new_from_slice(hash_bytes).ok()?;
    mac.update(challenge);
    Some(mac.finalize().into_bytes().into())
}

/// Server verifies the client's response against its stored PHC hash.
pub fn server_verify_response(
    stored_phc: &str,
    challenge: &[u8; 32],
    client_response: &[u8; 32],
    passphrase: &str,
) -> bool {
    let salt = match extract_salt(stored_phc) {
        Some(s) => s,
        None => return false,
    };

    let expected = match client_compute_response(passphrase, &salt, challenge) {
        Some(r) => r,
        None => return false,
    };

    expected.ct_eq(client_response).into()
}
