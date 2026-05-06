use std::fmt;
use std::path::PathBuf;
use rcgen::{CertificateParams, KeyPair};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Generate a self-signed Ed25519 certificate and private key.
pub fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>, String, String), Box<dyn std::error::Error>> {
    let mut params = CertificateParams::new(vec!["spud.local".to_string()])?;
    params.is_ca = rcgen::IsCa::NoCa;
    let key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519)?;

    let cert = params.self_signed(&key_pair)?;
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::try_from(key_pair.serialize_der())?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    Ok((cert_der.into(), key_der, cert_pem, key_pem))
}

/// Compute SHA-256 fingerprint of a certificate's DER encoding.
pub fn cert_fingerprint(cert: &CertificateDer<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(cert.as_ref());
    hasher.finalize().into()
}

/// Format a fingerprint as a hex string for display.
pub fn format_fingerprint(fingerprint: &[u8; 32]) -> String {
    hex::encode(fingerprint)
}

/// Load certificate and key from disk, or generate and persist new ones.
pub fn load_or_generate_certs() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), Box<dyn std::error::Error>> {
    let cert_path = cert_file_path();
    let key_path = key_file_path();

    if cert_path.exists() && key_path.exists() {
        let cert_pem = std::fs::read_to_string(&cert_path)?;
        let key_pem = std::fs::read_to_string(&key_path)?;

        let mut cert_reader = std::io::BufReader::new(cert_pem.as_bytes());
        let mut key_reader = std::io::BufReader::new(key_pem.as_bytes());

        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
            .filter_map(|r| r.ok())
            .collect();
        let keys: Vec<PrivateKeyDer<'static>> = rustls_pemfile::private_key(&mut key_reader)?
            .into_iter()
            .collect();

        if let (Some(cert), Some(key)) = (certs.into_iter().next(), keys.into_iter().next()) {
            return Ok((cert, key));
        }
    }

    let (cert, key, cert_pem, key_pem) = generate_self_signed_cert()?;
    if let Some(parent) = cert_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&cert_path, cert_pem)?;
    std::fs::write(&key_path, key_pem)?;

    Ok((cert, key))
}

fn cert_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("spud")
        .join("cert.pem")
}

fn key_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("spud")
        .join("key.pem")
}

/// TOFU certificate verifier that checks the SHA-256 fingerprint.
pub struct FingerprintVerifier {
    expected: [u8; 32],
}

impl FingerprintVerifier {
    pub fn new(expected: [u8; 32]) -> Self {
        Self { expected }
    }
}

impl fmt::Debug for FingerprintVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FingerprintVerifier")
            .field("expected", &hex::encode(&self.expected))
            .finish()
    }
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fingerprint = cert_fingerprint(end_entity);
        if fingerprint.ct_eq(&self.expected).into() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(Error::General("certificate fingerprint mismatch".to_string()))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("TLS 1.2 is disabled".to_string()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        let supported = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls13_signature(message, cert, dss, &supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}
