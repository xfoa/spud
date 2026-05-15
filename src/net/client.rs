use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
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

use crate::cert::cert_fingerprint;
use crate::config;
use crate::net::push_event;
use crate::net::Event;
use crate::net::NetEvent;
use crate::net::protocol::ControlMsg;
use crate::net::tls::build_client_config;

/// Errors that can occur when connecting to a server.
#[derive(Debug)]
pub enum ConnectError {
    /// The server's certificate fingerprint does not match the stored one.
    FingerprintMismatch([u8; 32]),
    /// A generic I/O or protocol error.
    Io(io::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::FingerprintMismatch(_) => {
                write!(f, "Server certificate fingerprint changed")
            }
            ConnectError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ConnectError {}

impl From<io::Error> for ConnectError {
    fn from(e: io::Error) -> Self {
        ConnectError::Io(e)
    }
}

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
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        let supported = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls13_signature(message, cert, dss, &supported)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::ED25519]
    }
}

/// Try TCP + TLS on each address in order. Returns the first successful
/// connection or the last error encountered.
async fn try_connect_addrs(
    addrs: &[SocketAddr],
    server_name: &rustls::pki_types::ServerName<'static>,
    connector: &TlsConnector,
) -> io::Result<(TlsStream<TcpStream>, SocketAddr)> {
    let mut last_err = None;
    for addr in addrs {
        match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(*addr)).await {
            Ok(Ok(tcp)) => match tokio::time::timeout(
                Duration::from_secs(10),
                connector.connect(server_name.clone(), tcp),
            )
            .await
            {
                Ok(Ok(tls)) => return Ok((tls, *addr)),
                Ok(Err(e)) => {
                    eprintln!("[spud] TLS handshake failed for {addr}: {e}");
                    last_err = Some(e);
                }
                Err(_) => {
                    eprintln!("[spud] TLS handshake timeout for {addr}");
                    last_err = Some(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "TLS handshake timeout",
                    ));
                }
            },
            Ok(Err(e)) => {
                eprintln!("[spud] TCP connect failed for {addr}: {e}");
                last_err = Some(e);
            }
            Err(_) => {
                eprintln!("[spud] TCP connect timeout for {addr}");
                last_err = Some(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "TCP connect timeout",
                ));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "no addresses found")
    }))
}

pub struct ClientConnection {
    udp_tx: mpsc::UnboundedSender<Event>,
    _shutdown: Arc<tokio::sync::Notify>,
    pub conn_id: u64,
    pub encrypt: bool,
    pub last_salt: Option<String>,
    pub screen_size: Option<(u16, u16)>,
    tcp_tx: mpsc::UnboundedSender<ControlMsg>,
    cipher: Option<Aes256Gcm>,
    udp_drop: Arc<AtomicU8>,
    batch_redundancy: Arc<AtomicU8>,
}

impl std::fmt::Debug for ClientConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConnection")
            .field("conn_id", &self.conn_id)
            .field("encrypt", &self.encrypt)
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
            last_salt: self.last_salt.clone(),
            screen_size: self.screen_size,
            tcp_tx: self.tcp_tx.clone(),
            cipher: self.cipher.clone(),
            udp_drop: self.udp_drop.clone(),
            batch_redundancy: self.batch_redundancy.clone(),
        }
    }
}

/// Send a batch of events over UDP.
async fn send_batch(
    batch: &[Event],
    history: &std::collections::VecDeque<Vec<Event>>,
    socket: &UdpSocket,
    conn_id: u64,
    encrypt: bool,
    cipher: Option<&Aes256Gcm>,
    seq: &AtomicU64,
) {
    let buf = Event::encode_batch(batch, history);
    let packet: Option<Vec<u8>> = if encrypt {
        if let Some(c) = cipher {
            let s = seq.fetch_add(1, Ordering::SeqCst);
            match crate::crypto::encrypt_event(c, s, &buf) {
                Ok(mut ct) => {
                    let mut p = Vec::with_capacity(16 + ct.len());
                    p.extend_from_slice(&conn_id.to_le_bytes());
                    p.extend_from_slice(&s.to_le_bytes());
                    p.append(&mut ct);
                    Some(p)
                }
                Err(e) => {
                    eprintln!("[spud] UDP encrypt failed for conn {conn_id}: {e}");
                    None
                }
            }
        } else {
            eprintln!("[spud] UDP encryption requested but no cipher available");
            None
        }
    } else {
        let mut p = Vec::with_capacity(8 + buf.len());
        p.extend_from_slice(&conn_id.to_le_bytes());
        p.extend_from_slice(&buf);
        Some(p)
    };
    if let Some(p) = packet {
        let _ = socket.send(&p).await;
    }
}

impl ClientConnection {
    /// Connect to a server.
    ///
    /// `saved_phc` is an optional previously-saved PHC string. If its salt matches the server's
    /// salt, the hash bytes are reused directly without requiring the plaintext passphrase.
    /// On success, returns `(ClientConnection, Option<String>)` where the option is the PHC
    /// string to save for future connections.
    pub async fn connect(
        host: &str,
        port: u16,
        addrs: Option<Vec<SocketAddr>>,
        client_encrypt: bool,
        client_require_auth: bool,
        passphrase: Option<String>,
        saved_phc: Option<String>,
        override_fingerprint: Option<[u8; 32]>,
        max_batch: u8,
        udp_drop_percent: u8,
        batch_redundancy: u8,
    ) -> Result<(Self, Option<String>), ConnectError> {
        let addrs: Vec<SocketAddr> = match addrs {
            Some(a) if !a.is_empty() => a,
            _ => tokio::net::lookup_host(format!("{}:{}", host, port))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
                .collect(),
        };

        if addrs.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no addresses found").into());
        }

        eprintln!("[spud] connect to {host}:{port} trying addresses: {addrs:?}");

        // Determine if we have a trusted fingerprint.
        // Check IP-based keys first (more specific), then hostname.
        let stored_fp: Option<[u8; 32]> = override_fingerprint.or_else(|| {
            let known = config::load_known_servers();
            addrs
                .iter()
                .find_map(|a| known.get(&format!("{}:{}", a.ip(), port)))
                .or_else(|| known.get(&format!("{}:{}", host, port)))
                .and_then(|s| hex::decode(s).ok())
                .and_then(|v| v.try_into().ok())
        });

        let server_name = rustls::pki_types::ServerName::try_from("spud.local")
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let (tls, addr) = if let Some(fp) = stored_fp {
            let config = build_client_config(fp);
            let connector = TlsConnector::from(config);
            match try_connect_addrs(&addrs, &server_name, &connector).await {
                Ok((tls, addr)) => {
                    eprintln!("[spud] connected via {addr} (trusted fingerprint)");
                    (tls, addr)
                }
                Err(original_err) => {
                    // TLS failed with stored fingerprint -- probe to see if it changed.
                    let provider = rustls::crypto::ring::default_provider();
                    let mut probe_config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                        .with_protocol_versions(&[&rustls::version::TLS13])
                        .expect("valid protocol versions")
                        .with_root_certificates(rustls::RootCertStore::empty())
                        .with_no_client_auth();
                    probe_config.dangerous().set_certificate_verifier(Arc::new(ProbeVerifier));
                    let probe_connector = TlsConnector::from(Arc::new(probe_config));
                    match try_connect_addrs(&addrs, &server_name, &probe_connector).await {
                        Ok((probe_tls, probe_addr)) => {
                            let (_, conn) = probe_tls.get_ref();
                            let certs = conn.peer_certificates().ok_or_else(|| {
                                io::Error::new(io::ErrorKind::InvalidData, "no peer certificates")
                            })?;
                            let cert = certs.first().ok_or_else(|| {
                                io::Error::new(io::ErrorKind::InvalidData, "empty peer certificates")
                            })?;
                            let new_fp = cert_fingerprint(cert);
                            if new_fp == fp {
                                // Same cert, some other TLS error; reuse probe connection.
                                eprintln!("[spud] connected via {probe_addr} (probe reuse, same fingerprint)");
                                (probe_tls, probe_addr)
                            } else {
                                return Err(ConnectError::FingerprintMismatch(new_fp));
                            }
                        }
                        Err(probe_err) => {
                            eprintln!("[spud] probe after failed TLS also failed: {probe_err}");
                            return Err(ConnectError::Io(original_err));
                        }
                    }
                }
            }
        } else {
            // No stored fingerprint: probe, trust, and use the probe connection directly.
            let provider = rustls::crypto::ring::default_provider();
            let mut probe_config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .expect("valid protocol versions")
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            probe_config.dangerous().set_certificate_verifier(Arc::new(ProbeVerifier));
            let probe_connector = TlsConnector::from(Arc::new(probe_config));
            let (probe_tls, probe_addr) = try_connect_addrs(&addrs, &server_name, &probe_connector)
                .await
                .map_err(ConnectError::Io)?;
            let (_, conn) = probe_tls.get_ref();
            let certs = conn.peer_certificates().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "no peer certificates")
            })?;
            let cert = certs.first().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "empty peer certificates")
            })?;
            let new_fp = cert_fingerprint(cert);
            config::trust_server(host, port, new_fp);
            config::trust_server(&probe_addr.ip().to_string(), port, new_fp);
            eprintln!("[spud] connected via {probe_addr} (after probe)");
            (probe_tls, probe_addr)
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

        let (mut framed, udp_socket, conn_id, server_encrypt, phc_to_save, screen_width, screen_height) =
            Self::handshake(tls, addr, client_require_auth, passphrase, saved_phc).await?;
        if client_encrypt != server_encrypt {
            return Err(ConnectError::Io(io::Error::new(io::ErrorKind::InvalidData, if client_encrypt { 
                "This server doesn't use encryption, disable it in Security to connect"
            } else {
                "This server uses encryption, enable it in Security to connect" 
            })));
        }
        let encrypt = client_encrypt;
        let cipher = encrypt.then(|| client_cipher).flatten();
        if encrypt && cipher.is_none() {
            return Err(ConnectError::Io(io::Error::new(io::ErrorKind::Other, "Failed to derive encryption keys")));
        }

        let shutdown = Arc::new(tokio::sync::Notify::new());
        let (udp_tx, mut udp_rx) = mpsc::unbounded_channel::<Event>();
        let (tcp_tx, mut tcp_rx) = mpsc::unbounded_channel::<ControlMsg>();

        // UDP sender task with batching
        let udp_socket = Arc::new(udp_socket);
        let seq = AtomicU64::new(1);
        let udp_socket_clone = udp_socket.clone();
        let conn_id_clone = conn_id;
        let encrypt_clone = encrypt;
        let cipher_clone = cipher.clone();
        let udp_drop = Arc::new(AtomicU8::new(udp_drop_percent));
        let udp_drop_clone = udp_drop.clone();
        let batch_redundancy = Arc::new(AtomicU8::new(batch_redundancy));
        let batch_redundancy_clone = batch_redundancy.clone();
        tokio::spawn(async move {
            let max_batch = max_batch.max(1) as usize;
            const BATCH_TIMEOUT_MS: u64 = 1;
            let mut batch: Vec<Event> = Vec::with_capacity(max_batch);
            let mut history: std::collections::VecDeque<Vec<Event>> = std::collections::VecDeque::new();
            let flush_deadline = tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_TIMEOUT_MS));
            tokio::pin!(flush_deadline);

            loop {
                let drop_pct = udp_drop_clone.load(Ordering::Relaxed);
                let redundancy = batch_redundancy_clone.load(Ordering::Relaxed) as usize;
                tokio::select! {
                    Some(event) = udp_rx.recv() => {
                        batch.push(event);
                        if batch.len() >= max_batch {
                            if drop_pct == 0 || fastrand::u8(0..100) >= drop_pct {
                                send_batch(&batch, &history, &udp_socket_clone, conn_id_clone, encrypt_clone, cipher_clone.as_ref(), &seq).await;
                            }
                            if redundancy > 0 {
                                history.push_back(batch.clone());
                                while history.len() > redundancy {
                                    history.pop_front();
                                }
                            } else {
                                history.clear();
                            }
                            batch.clear();
                            flush_deadline.set(tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_TIMEOUT_MS)));
                        }
                    }
                    _ = &mut flush_deadline => {
                        if !batch.is_empty() {
                            if drop_pct == 0 || fastrand::u8(0..100) >= drop_pct {
                                send_batch(&batch, &history, &udp_socket_clone, conn_id_clone, encrypt_clone, cipher_clone.as_ref(), &seq).await;
                            }
                            if redundancy > 0 {
                                history.push_back(batch.clone());
                                while history.len() > redundancy {
                                    history.pop_front();
                                }
                            } else {
                                history.clear();
                            }
                            batch.clear();
                        }
                        flush_deadline.set(tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_TIMEOUT_MS)));
                    }
                    else => break,
                }
            }

            // Flush any remaining events on shutdown
            let drop_pct = udp_drop_clone.load(Ordering::Relaxed);
            if !batch.is_empty() {
                if drop_pct == 0 || fastrand::u8(0..100) >= drop_pct {
                    send_batch(&batch, &history, &udp_socket_clone, conn_id_clone, encrypt_clone, cipher_clone.as_ref(), &seq).await;
                }
            }
        });

        // TLS read task for liveness / disconnect detection + outbound control
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
                    Some(msg) = tcp_rx.recv() => {
                        let bytes = match postcard::to_allocvec(&msg) {
                            Ok(b) => b,
                            Err(_) => continue,
                        };
                        if framed.send(bytes.into()).await.is_err() {
                            break;
                        }
                    }
                    _ = shutdown_clone.notified() => break,
                }
            }
            push_event(NetEvent::Disconnected);
        });

        Ok((Self {
            udp_tx,
            tcp_tx,
            _shutdown: shutdown,
            conn_id,
            encrypt,
            last_salt: phc_to_save.clone(),
            screen_size: Some((screen_width, screen_height)),
            cipher,
            udp_drop,
            batch_redundancy,
        }, phc_to_save))
    }

    async fn handshake(
        tls: TlsStream<TcpStream>,
        server_addr: SocketAddr,
        client_require_auth: bool,
        passphrase: Option<String>,
        saved_phc: Option<String>,
    ) -> io::Result<(Framed<TlsStream<TcpStream>, LengthDelimitedCodec>, UdpSocket, u64, bool, Option<String>, u16, u16)> {
        let mut framed = Framed::new(tls, LengthDelimitedCodec::new());

        let frame = tokio::time::timeout(Duration::from_secs(5), framed.next())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "server closed connection"))?
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let msg: ControlMsg = postcard::from_bytes(&frame)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut msg = msg;
        let mut phc_to_save = None;

        // Handle auth challenge if present
        if let ControlMsg::AuthChallenge { nonce, salt } = msg {
            if !client_require_auth {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Server requires authentication but it is disabled on this client",
                ));
            }
            let salt_b64 = crate::config::encode_salt_bytes(&salt);
            let hmac = match saved_phc {
                Some(ref phc) => match crate::net::auth::client_compute_response_from_phc(phc, &salt_b64, &nonce) {
                    Some(hmac) => {
                        phc_to_save = Some(phc.clone());
                        Some(hmac)
                    }
                    None => None,
                }
                None => None,
            };

            let hmac = match hmac {
                Some(hmac) => hmac,
                None => {
                    let passphrase = passphrase.ok_or_else(|| {
                        io::Error::new(io::ErrorKind::PermissionDenied, "Authentication required but no passphrase provided")
                    })?;
                    phc_to_save = crate::config::hash_passphrase_with_salt(&passphrase, &salt_b64);
                    crate::net::auth::client_compute_response(&passphrase, &salt_b64, &nonce)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Authentication error: failed to compute auth response"))?
                }
            };
            let response = ControlMsg::AuthResponse { hmac };
            let bytes = postcard::to_allocvec(&response)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            framed.send(bytes.into()).await
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            let frame = tokio::time::timeout(Duration::from_secs(5), framed.next())
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Authentication error: auth result timeout"))?
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Authentication failed: server closed connection during auth"))?
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            msg = postcard::from_bytes(&frame)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            if let ControlMsg::AuthResult { ok: false } = msg {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed: incorrect password"));
            }
            if let ControlMsg::AuthResult { ok: true } = msg {
                // Read SessionInit next
                let frame = tokio::time::timeout(Duration::from_secs(5), framed.next())
                    .await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Authentication failed: handshake timeout after auth"))?
                    .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Authentication failed: server closed connection after auth"))?
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                msg = postcard::from_bytes(&frame)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            }
        }

        let (conn_id, server_encrypt, server_auth, screen_width, screen_height) = match msg {
            ControlMsg::SessionInit { conn_id, encrypt, auth, screen_width, screen_height, .. } => (conn_id, encrypt, auth, screen_width, screen_height),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Connect failed: expected SessionInit")),
        };

        if client_require_auth != server_auth {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                if client_require_auth {
                    "Server does not require a passphrase, disable it in Security to connect"
                } else {
                    "Server requires a passphrase, enable it in Security to connect"
                },
            ));
        }

        let udp = UdpSocket::bind("0.0.0.0:0").await?;
        udp.connect(server_addr).await?;

        Ok((framed, udp, conn_id, server_encrypt, phc_to_save, screen_width, screen_height))
    }

    pub fn send(&self, event: &Event) {
        let _ = self.udp_tx.send(event.clone());
    }

    pub fn send_control(&self, msg: ControlMsg) {
        let _ = self.tcp_tx.send(msg);
    }

    pub fn set_udp_drop_percent(&self, percent: u8) {
        self.udp_drop.store(percent.min(100), Ordering::Relaxed);
    }

    pub fn set_batch_redundancy(&self, count: u8) {
        self.batch_redundancy.store(count, Ordering::Relaxed);
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        self._shutdown.notify_one();
    }
}
