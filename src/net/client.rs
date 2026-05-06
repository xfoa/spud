use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroize;

use aes_gcm::aead::KeyInit;
use aes_gcm::Aes256Gcm;
use iced::futures::{SinkExt, StreamExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::cert::{cert_fingerprint, FingerprintVerifier};
use crate::config;
use crate::net::push_event;
use crate::net::Event;
use crate::net::NetEvent;
use crate::net::protocol::ControlMsg;
use crate::net::tls::build_client_config;

/// A verifier that accepts any certificate and records its fingerprint.
struct ProbeVerifier;

impl std::fmt::Debug for ProbeVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeVerifier").finish()
    }
}

impl rustls::client::danger::ServerCertVerifier for ProbeVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.2 is disabled".to_string()))
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::ED25519, rustls::SignatureScheme::ECDSA_NISTP256_SHA256]
    }
}

pub struct ClientConnection {
    udp_tx: mpsc::UnboundedSender<Event>,
    _shutdown: Arc<tokio::sync::Notify>,
    pub conn_id: u64,
    pub encrypt: bool,
    pub key_timeout_ms: u16,
    cipher: Option<Aes256Gcm>,
}

impl std::fmt::Debug for ClientConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConnection")
            .field("conn_id", &self.conn_id)
            .field("encrypt", &self.encrypt)
            .field("key_timeout_ms", &self.key_timeout_ms)
            .finish_non_exhaustive()
    }
}

impl Clone for ClientConnection {
    fn clone(&self) -> Self {
        Self {
            udp_tx: self.udp_tx.clone(),
            _shutdown: self._shutdown.clone(),
            conn_id: self.conn_id,
            encrypt: self.encrypt,
            key_timeout_ms: self.key_timeout_ms,
            cipher: self.cipher.clone(),
        }
    }
}

impl ClientConnection {
    pub async fn connect(host: &str, port: u16, client_encrypt: bool) -> io::Result<Self> {
        let addr = tokio::net::lookup_host(format!("{}:{}", host, port))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no addresses found"))?;

        // Determine if we have a trusted fingerprint
        let fingerprint = config::load_known_servers()
            .get(&format!("{}:{}", host, port))
            .and_then(|s| hex::decode(s).ok())
            .and_then(|v| v.try_into().ok());

        let tls = match fingerprint {
            Some(fp) => {
                let config = build_client_config(fp);
                let connector = TlsConnector::from(config);
                let server_name = rustls::pki_types::ServerName::try_from("spud.local")
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
                let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;
                tokio::time::timeout(Duration::from_secs(10), connector.connect(server_name.clone(), tcp))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TLS handshake timeout"))??
            }
            None => {
                // Probe to get fingerprint
                let provider = rustls::crypto::ring::default_provider();
                let mut probe_config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                    .with_protocol_versions(&[&rustls::version::TLS13])
                    .expect("valid protocol versions")
                    .with_root_certificates(rustls::RootCertStore::empty())
                    .with_no_client_auth();
                probe_config.dangerous().set_certificate_verifier(Arc::new(ProbeVerifier));
                let connector = TlsConnector::from(Arc::new(probe_config));
                let server_name = rustls::pki_types::ServerName::try_from("spud.local")
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
                let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;
                let tls = tokio::time::timeout(Duration::from_secs(10), connector.connect(server_name.clone(), tcp))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TLS handshake timeout"))??;

                // Extract fingerprint
                let (_, conn) = tls.get_ref();
                let certs = conn.peer_certificates()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no peer certificates"))?;
                let cert = certs.first()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty peer certificates"))?;
                let fp = cert_fingerprint(cert);
                config::trust_server(host, port, fp);

                // Reconnect with trusted fingerprint
                let config = build_client_config(fp);
                let connector = TlsConnector::from(config);
                let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;
                tokio::time::timeout(Duration::from_secs(10), connector.connect(server_name.clone(), tcp))
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TLS handshake timeout"))??
            }
        };

        // Derive UDP keys from TLS exporter before consuming tls in handshake
        let client_cipher = {
            let (_, conn) = tls.get_ref();
            let mut exported = [0u8; 64];
            match conn.export_keying_material(&mut exported, b"spud/udp/keys/v1", Some(b"")) {
                Ok(_) => {
                    let keys = crate::crypto::derive_udp_keys(&exported);
                    exported.zeroize();
                    Aes256Gcm::new_from_slice(&keys.client_write).ok()
                }
                Err(e) => {
                    eprintln!("[spud] TLS key export failed: {e}");
                    None
                }
            }
        };

        let (mut framed, udp_socket, conn_id, server_encrypt, key_timeout_ms) = Self::handshake(tls, addr).await?;
        let encrypt = client_encrypt && server_encrypt;
        let cipher = encrypt.then(|| client_cipher).flatten();

        let shutdown = Arc::new(tokio::sync::Notify::new());
        let (udp_tx, mut udp_rx) = mpsc::unbounded_channel::<Event>();

        // UDP sender task
        let udp_socket = Arc::new(udp_socket);
        let seq = AtomicU64::new(1);
        let udp_socket_clone = udp_socket.clone();
        let conn_id_clone = conn_id;
        let encrypt_clone = encrypt;
        let cipher_clone = cipher.clone();
        tokio::spawn(async move {
            while let Some(event) = udp_rx.recv().await {
                let buf = event.encode();
                let packet = if encrypt_clone {
                    if let Some(ref c) = cipher_clone {
                        let s = seq.fetch_add(1, Ordering::SeqCst);
                        match crate::crypto::encrypt_event(c, s, &buf) {
                            Ok(mut ct) => {
                                let mut p = Vec::with_capacity(16 + ct.len());
                                p.extend_from_slice(&conn_id_clone.to_le_bytes());
                                p.extend_from_slice(&s.to_le_bytes());
                                p.append(&mut ct);
                                p
                            }
                            Err(_) => {
                                let mut p = Vec::with_capacity(8 + buf.len());
                                p.extend_from_slice(&conn_id_clone.to_le_bytes());
                                p.extend_from_slice(&buf);
                                p
                            }
                        }
                    } else {
                        let mut p = Vec::with_capacity(8 + buf.len());
                        p.extend_from_slice(&conn_id_clone.to_le_bytes());
                        p.extend_from_slice(&buf);
                        p
                    }
                } else {
                    let mut p = Vec::with_capacity(8 + buf.len());
                    p.extend_from_slice(&conn_id_clone.to_le_bytes());
                    p.extend_from_slice(&buf);
                    p
                };
                let _ = udp_socket_clone.send(&packet).await;
            }
        });

        // TLS read task for liveness / disconnect detection
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        let hb = ControlMsg::Keepalive;
                        let bytes = match postcard::to_allocvec(&hb) {
                            Ok(b) => b,
                            Err(_) => break,
                        };
                        if framed.send(bytes.into()).await.is_err() {
                            break;
                        }
                    }
                    msg = framed.next() => {
                        match msg {
                            Some(Ok(_)) => {}
                            _ => break,
                        }
                    }
                    _ = shutdown_clone.notified() => break,
                }
            }
            push_event(NetEvent::Disconnected);
        });

        Ok(Self {
            udp_tx,
            _shutdown: shutdown,
            conn_id,
            encrypt,
            key_timeout_ms,
            cipher,
        })
    }

    async fn handshake(
        tls: TlsStream<TcpStream>,
        server_addr: SocketAddr,
    ) -> io::Result<(Framed<TlsStream<TcpStream>, LengthDelimitedCodec>, UdpSocket, u64, bool, u16)> {
        let mut framed = Framed::new(tls, LengthDelimitedCodec::new());

        let frame = tokio::time::timeout(Duration::from_secs(5), framed.next())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "server closed connection"))?
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let msg: ControlMsg = postcard::from_bytes(&frame)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let (conn_id, server_encrypt, key_timeout_ms) = match msg {
            ControlMsg::SessionInit { conn_id, encrypt, key_timeout_ms, .. } => (conn_id, encrypt, key_timeout_ms),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "expected SessionInit")),
        };

        let udp = UdpSocket::bind("0.0.0.0:0").await?;
        udp.connect(server_addr).await?;

        Ok((framed, udp, conn_id, server_encrypt, key_timeout_ms))
    }

    pub fn send(&self, event: &Event) {
        let _ = self.udp_tx.send(event.clone());
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        self._shutdown.notify_one();
    }
}
