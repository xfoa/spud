use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroize;

use aes_gcm::aead::KeyInit;
use aes_gcm::Aes256Gcm;
use iced::futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_rustls::TlsAcceptor;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;

use crate::net::protocol::ControlMsg;
use crate::net::tls::build_server_config;
use crate::session::{generate_session, SessionState, SessionTable};

pub struct ServerListener {
    shutdown: Arc<tokio::sync::Notify>,
    handle: tokio::task::JoinHandle<()>,
}

impl ServerListener {
    pub async fn bind(
        addr: &str,
        port: u16,
        key_timeout_ms: u16,
        require_auth: bool,
        passphrase_hash: String,
        encrypt_udp: bool,
    ) -> io::Result<Self> {
        let (cert, key) = crate::cert::load_or_generate_certs()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let config = build_server_config(cert, key);
        let acceptor = TlsAcceptor::from(config);

        let tcp = TcpListener::bind((addr, port)).await?;
        let udp = UdpSocket::bind((addr, port)).await?;

        let shutdown = Arc::new(tokio::sync::Notify::new());
        let sessions: Arc<SessionTable> = Arc::new(SessionTable::new());

        let s = shutdown.clone();
        let handle = tokio::spawn(run_server(tcp, udp, acceptor, s, require_auth, passphrase_hash, encrypt_udp, key_timeout_ms, sessions));

        Ok(Self { shutdown, handle })
    }
}

impl Drop for ServerListener {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
        self.handle.abort();
    }
}

async fn run_server(
    tcp: TcpListener,
    udp: UdpSocket,
    acceptor: TlsAcceptor,
    shutdown: Arc<tokio::sync::Notify>,
    require_auth: bool,
    passphrase_hash: String,
    encrypt_udp: bool,
    key_timeout_ms: u16,
    sessions: Arc<SessionTable>,
) {
    let cancel = CancellationToken::new();
    let mut buf = vec![0u8; 2048];
    loop {
        tokio::select! {
            _ = shutdown.notified() => break,
            result = tcp.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        let acceptor = acceptor.clone();
                        let sessions = sessions.clone();
                        let hash = passphrase_hash.clone();
                        let child_cancel = cancel.child_token();
                        tokio::spawn(handle_client(
                            stream, peer, acceptor, sessions, require_auth, hash, encrypt_udp, key_timeout_ms, child_cancel,
                        ));
                    }
                    Err(e) => {
                        eprintln!("[spud] tcp accept: {e}");
                    }
                }
            }
            result = udp.recv_from(&mut buf) => {
                match result {
                    Ok((n, src)) => {
                        if n < 8 {
                            continue;
                        }
                        let conn_id = u64::from_le_bytes(buf[..8].try_into().unwrap());
                        let payload = &buf[8..n];

                        let mut should_remove = false;
                        if let Some(mut session) = sessions.get_mut(&conn_id) {
                            session.last_activity = std::time::Instant::now();
                            session.src_addr = src;

                            let plaintext: Option<Vec<u8>> = if session.encrypt {
                                if n >= 16 + 16 {
                                    let seq = u64::from_le_bytes(buf[8..16].try_into().unwrap());
                                    if !session.replay_window.is_valid(seq) {
                                        eprintln!("[spud] UDP replay/duplicate seq {seq} for conn {conn_id}, dropping");
                                        None
                                    } else if let Some(ref keys) = session.keys {
                                        let nonce_ct = &buf[16..n];
                                        let cipher = Aes256Gcm::new_from_slice(&keys.server_read).unwrap();
                                        crate::crypto::decrypt_event(&cipher, seq, nonce_ct)
                                    } else {
                                        eprintln!("[spud] encrypted session missing keys, dropping");
                                        None
                                    }
                                } else {
                                    eprintln!("[spud] UDP packet too short for encryption, dropping");
                                    None
                                }
                            } else {
                                Some(payload.to_vec())
                            };

                            if let Some(pt) = plaintext {
                                session.record_decrypt_success();
                                if let Some(event) = crate::net::Event::decode(&pt) {
                                    // TODO: feed to input replay instead of just printing
                                    println!("[server] {src}: {event:?}");
                                }
                            } else if session.encrypt {
                                should_remove = session.record_decrypt_failure();
                                if should_remove {
                                    eprintln!("[spud] UDP too many failed decrypts for conn {conn_id}, removing session");
                                }
                            }
                        }
                        if should_remove {
                            sessions.remove(&conn_id);
                        }
                    }
                    Err(e) => {
                        eprintln!("[spud] udp recv: {e}");
                    }
                }
            }
        }
    }
    cancel.cancel();
}

async fn handle_client(
    stream: TcpStream,
    peer: SocketAddr,
    acceptor: TlsAcceptor,
    sessions: Arc<SessionTable>,
    require_auth: bool,
    passphrase_hash: String,
    encrypt_udp: bool,
    key_timeout_ms: u16,
    cancel: CancellationToken,
) {
    let tls = match acceptor.accept(stream).await {
        Ok(tls) => tls,
        Err(e) => {
            eprintln!("[spud] tls accept: {e}");
            return;
        }
    };

    // Derive UDP keys from TLS exporter before consuming tls in framed
    let keys = {
        let (_, conn) = tls.get_ref();
        let mut exported = [0u8; 64];
        match conn.export_keying_material(&mut exported, b"spud/udp/keys/v1", Some(b"")) {
            Ok(_) => {
                let udp_keys = crate::crypto::derive_udp_keys(&exported);
                exported.zeroize();
                Some(crate::session::SessionKeys {
                    server_read: udp_keys.client_write,
                    server_write: udp_keys.server_write,
                })
            }
            Err(e) => {
                eprintln!("[spud] TLS key export failed: {e}");
                None
            }
        }
    };

    let mut framed = Framed::new(tls, LengthDelimitedCodec::new());

    // Auth challenge-response
    if require_auth && !passphrase_hash.is_empty() {
        let challenge = crate::net::auth::generate_challenge();
        let salt = crate::config::extract_salt(&passphrase_hash).unwrap_or_default();
        let challenge_msg = ControlMsg::AuthChallenge { nonce: challenge, salt };
        let bytes = match postcard::to_allocvec(&challenge_msg) {
            Ok(b) => b,
            Err(_) => return,
        };
        if framed.send(bytes.into()).await.is_err() {
            return;
        }

        let response = match tokio::time::timeout(Duration::from_secs(10), framed.next()).await {
            Ok(Some(Ok(frame))) => frame,
            _ => return,
        };
        let auth_ok = match postcard::from_bytes::<ControlMsg>(&response) {
            Ok(ControlMsg::AuthResponse { hmac }) => {
                let ok = crate::net::auth::server_verify_response(&passphrase_hash, &challenge, &hmac);
                if !ok {
                    eprintln!("[spud] auth failed for {peer}: response mismatch");
                }
                ok
            }
            Ok(_) => {
                eprintln!("[spud] auth failed for {peer}: expected AuthResponse, got other message");
                false
            }
            Err(e) => {
                eprintln!("[spud] auth failed for {peer}: failed to parse response: {e}");
                false
            }
        };

        let result_msg = ControlMsg::AuthResult { ok: auth_ok };
        let bytes = match postcard::to_allocvec(&result_msg) {
            Ok(b) => b,
            Err(_) => return,
        };
        let _ = framed.send(bytes.into()).await;
        if !auth_ok {
            return;
        }
    }

    let (uuid, conn_id) = generate_session();
    let session = SessionState::new(encrypt_udp, keys, peer);
    sessions.insert(conn_id, session);

    let init = ControlMsg::SessionInit { conn_id, uuid, encrypt: encrypt_udp, key_timeout_ms };
    let bytes = match postcard::to_allocvec(&init) {
        Ok(b) => b,
        Err(_) => {
            sessions.remove(&conn_id);
            return;
        }
    };
    if framed.send(bytes.into()).await.is_err() {
        sessions.remove(&conn_id);
        return;
    }

    // Keep TLS alive until disconnect
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            msg = framed.next() => {
                match msg {
                    Some(Ok(bytes)) => {
                        if let Ok(ControlMsg::Keepalive) = postcard::from_bytes(&bytes) {
                            // Update activity
                            if let Some(mut s) = sessions.get_mut(&conn_id) {
                                s.last_activity = std::time::Instant::now();
                            }
                        }
                    }
                    _ => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                // Session timeout check
                let now = std::time::Instant::now();
                if let Some(s) = sessions.get(&conn_id) {
                    if now.duration_since(s.last_activity) > Duration::from_secs(300) {
                        break;
                    }
                }
            }
        }
    }

    sessions.remove(&conn_id);
}

