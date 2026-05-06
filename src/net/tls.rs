use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::cert::FingerprintVerifier;

pub fn build_server_config(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> Arc<ServerConfig> {
    let provider = rustls::crypto::ring::default_provider();
    let config = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("valid protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .expect("valid cert/key pair");

    let mut config = config;
    config.alpn_protocols = vec![b"spud/1".to_vec()];

    Arc::new(config)
}

pub fn build_client_config(trusted_fingerprint: [u8; 32]) -> Arc<ClientConfig> {
    let provider = rustls::crypto::ring::default_provider();
    let root_store = RootCertStore::empty();
    let mut config = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("valid protocol versions")
        .with_root_certificates(root_store)
        .with_no_client_auth();

    config.alpn_protocols = vec![b"spud/1".to_vec()];

    config.dangerous().set_certificate_verifier(
        Arc::new(FingerprintVerifier::new(trusted_fingerprint)),
    );

    Arc::new(config)
}
