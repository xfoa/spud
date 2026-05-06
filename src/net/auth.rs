use hmac::{Hmac, Mac};
use rand_core::OsRng;
use rand_core::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::config::hash_passphrase_with_salt;

/// Generate a 32-byte random challenge.
pub fn generate_challenge() -> [u8; 32] {
    let mut challenge = [0u8; 32];
    OsRng.fill_bytes(&mut challenge);
    challenge
}

/// Client computes HMAC-SHA256(Argon2(passphrase, salt), challenge).
/// The salt is extracted from the server's PHC string.
pub fn client_compute_response(passphrase: &str, phc: &str, challenge: &[u8; 32]) -> Option<[u8; 32]> {
    let parsed = argon2::password_hash::PasswordHash::new(phc).ok()?;
    let salt = parsed.salt.map(|s| s.as_str().to_string())?;
    let hash = hash_passphrase_with_salt(passphrase, &salt)?;
    let parsed = argon2::password_hash::PasswordHash::new(&hash).ok()?;
    let hash = parsed.hash?;
    let hash_bytes = hash.as_bytes();

    let mut mac = Hmac::<Sha256>::new_from_slice(hash_bytes).ok()?;
    mac.update(challenge);
    Some(mac.finalize().into_bytes().into())
}

/// Server computes the expected response from its stored PHC hash.
/// The PHC string's hash output is used directly as the HMAC key,
/// so the plaintext passphrase is not required.
pub fn server_compute_expected(stored_phc: &str, challenge: &[u8; 32]) -> Option<[u8; 32]> {
    let parsed = argon2::password_hash::PasswordHash::new(stored_phc).ok()?;
    let hash = parsed.hash?;
    let hash_bytes = hash.as_bytes();

    let mut mac = Hmac::<Sha256>::new_from_slice(hash_bytes).ok()?;
    mac.update(challenge);
    Some(mac.finalize().into_bytes().into())
}

/// Server verifies the client's response against its stored PHC hash.
pub fn server_verify_response(stored_phc: &str, challenge: &[u8; 32], client_response: &[u8; 32]) -> bool {
    let expected = match server_compute_expected(stored_phc, challenge) {
        Some(r) => r,
        None => return false,
    };
    expected.ct_eq(client_response).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hash_passphrase;

    #[test]
    fn auth_roundtrip() {
        let passphrase = "correct horse battery staple";
        let stored_phc = hash_passphrase(passphrase);
        assert!(!stored_phc.is_empty(), "PHC hash should not be empty");

        let challenge = generate_challenge();
        let client_response = client_compute_response(passphrase, &stored_phc, &challenge)
            .expect("client should compute response");
        let expected = server_compute_expected(&stored_phc, &challenge)
            .expect("server should compute expected");

        assert_eq!(client_response, expected, "client and server HMACs should match");
        assert!(server_verify_response(&stored_phc, &challenge, &client_response));
    }

    #[test]
    fn auth_wrong_passphrase_fails() {
        let stored_phc = hash_passphrase("right password");
        let challenge = generate_challenge();
        let client_response = client_compute_response("wrong password", &stored_phc, &challenge)
            .expect("client should compute response even with wrong password");

        assert!(!server_verify_response(&stored_phc, &challenge, &client_response));
    }
}
