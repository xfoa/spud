use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use zeroize::Zeroize;

use aes_gcm::aead::KeyInit;
use aes_gcm::Aes256Gcm;
use iced::futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio_rustls::TlsAcceptor;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;

#[cfg(target_os = "linux")]
use crate::input::InputInjector;
use crate::net::protocol::ControlMsg;
use crate::net::tls::build_server_config;
use crate::session::{generate_session, SessionState, SessionTable};

pub struct ServerListener {
    shutdown: Arc<tokio::sync::Notify>,
    handle: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
    #[cfg(target_os = "linux")]
    helper_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl ServerListener {
    pub async fn bind(
        addr: &str,
        port: u16,
        key_timeout_ms: u16,
        require_auth: bool,
        passphrase_hash: String,
        encrypt_udp: bool,
        batch_history_multiplier: u8,
    ) -> io::Result<Self> {
        let (cert, key) = crate::cert::load_or_generate_certs()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let config = build_server_config(cert, key);
        let acceptor = TlsAcceptor::from(config);

        let tcp_socket = TcpSocket::new_v4()?;
        tcp_socket.set_reuseaddr(true)?;
        let tcp_addr = std::net::SocketAddr::new(
            addr.parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?,
            port,
        );
        tcp_socket.bind(tcp_addr)?;
        let tcp = tcp_socket.listen(128)?;

        let udp = UdpSocket::bind((addr, port)).await?;

        let shutdown = Arc::new(tokio::sync::Notify::new());
        let sessions: Arc<SessionTable> = Arc::new(SessionTable::new());
        let (screen_width, screen_height) = get_screen_size();

        #[cfg(target_os = "linux")]
        let helper_cancel: Arc<std::sync::atomic::AtomicBool> = Arc::new(std::sync::atomic::AtomicBool::new(false));

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let injector: Arc<OnceLock<crate::input::InputInjector>> = Arc::new(OnceLock::new());
        #[cfg(target_os = "linux")]
        {
            match try_direct_injector(screen_width, screen_height) {
                Some(inj) => {
                    let _ = injector.set(inj);
                }
                None => {
                    eprintln!("[spud] Permission denied opening /dev/uinput. Starting privileged helper...");
                    let slot = injector.clone();
                    let cancel = helper_cancel.clone();
                    let _ = spawn_helper_injector(screen_width, screen_height, slot, cancel);
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            match crate::input::InputInjector::new(screen_width, screen_height) {
                Ok(inj) => {
                    let _ = injector.set(inj);
                }
                Err(e) => {
                    eprintln!("[spud] Failed to create macOS input injector: {e}");
                }
            }
        }

        let s = shutdown.clone();
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let handle = tokio::spawn(run_server(tcp, udp, acceptor, s, require_auth, passphrase_hash, encrypt_udp, key_timeout_ms, sessions, screen_width, screen_height, injector, c, batch_history_multiplier));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let handle = tokio::spawn(run_server(tcp, udp, acceptor, s, require_auth, passphrase_hash, encrypt_udp, key_timeout_ms, sessions, screen_width, screen_height, c, batch_history_multiplier));

        Ok(Self {
            shutdown,
            handle,
            cancel,
            #[cfg(target_os = "linux")]
            helper_cancel: Some(helper_cancel),
        })
    }
}

impl Drop for ServerListener {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(ref cancel) = self.helper_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.cancel.cancel();
        self.shutdown.notify_waiters();
        self.handle.abort();
        // The tokio task will be dropped on the next runtime tick.
        // We must not block here because this Drop may run on the tokio
        // runtime thread, which would prevent the task from being polled
        // and dropped.
    }
}

fn get_screen_size() -> (u16, u16) {
    #[cfg(target_os = "linux")]
    {
        use x11rb::connection::Connection;
        use x11rb::rust_connection::RustConnection;
        if let Ok((conn, screen_num)) = RustConnection::connect(None) {
            if let Some(screen) = conn.setup().roots.get(screen_num) {
                return (screen.width_in_pixels, screen.height_in_pixels);
            }
        }
        return (1920, 1080);
    }
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::CGDisplay;
        let main = CGDisplay::main();
        let bounds = main.bounds();
        return (bounds.size.width as u16, bounds.size.height as u16);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        return (1920, 1080);
    }
}

#[cfg(target_os = "linux")]
fn wire_to_platform_button(wire: u8) -> u16 {
    crate::input::wire_to_linux_button(wire)
}

#[cfg(target_os = "macos")]
fn wire_to_platform_button(wire: u8) -> u16 {
    // macOS injector translates wire codes internally.
    wire as u16
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn wire_to_platform_button(wire: u8) -> u16 {
    wire as u16
}

#[cfg(target_os = "linux")]
fn try_direct_injector(screen_width: u16, screen_height: u16) -> Option<InputInjector> {
    match crate::input::InputInjector::new(screen_width, screen_height) {
        Ok(inj) => Some(inj),
        Err(e) => {
            eprintln!("[spud] Failed to create input injector: {e}");
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_helper_injector(
    screen_width: u16,
    screen_height: u16,
    slot: Arc<OnceLock<crate::input::InputInjector>>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let socket_path = format!("/tmp/spud-input-{}.sock", std::process::id());
        let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("spud"));
        let exe_str = exe.to_string_lossy();
        let mut child = match std::process::Command::new("pkexec")
            .arg(&*exe_str)
            .arg("injection-helper")
            .arg(&socket_path)
            .arg(screen_width.to_string())
            .arg(screen_height.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[spud] Failed to spawn pkexec helper: {e}");
                return;
            }
        };

        let start = std::time::Instant::now();
        loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.try_wait();
                return;
            }

            if let Ok(Some(status)) = child.try_wait() {
                eprintln!("[spud] Helper exited early with status: {status}");
                break;
            }

            if std::path::Path::new(&socket_path).exists() {
                match crate::input::InputInjector::new_ipc(&socket_path) {
                    Ok(mut inj) => {
                        inj.helper = Some(child);
                        eprintln!("[spud] Input injector created via privileged helper.");
                        if let Err(_) = slot.set(inj) {
                            eprintln!("[spud] Warning: injector slot already initialized");
                        }
                        return;
                    }
                    Err(e) => {
                        eprintln!("[spud] new_ipc retry failed: {e}");
                    }
                }
            }

            let elapsed = start.elapsed().as_secs();
            if elapsed > 0 && elapsed % 5 == 0 {
                eprintln!("[spud] Still waiting for privileged helper... ({elapsed}s elapsed)");
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        eprintln!("[spud] Failed to connect to helper.");
        let _ = child.kill();
        let _ = child.try_wait();
        eprintln!("[spud] Input events will be logged only, not injected.");
    })
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
    screen_width: u16,
    screen_height: u16,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    injector: Arc<OnceLock<crate::input::InputInjector>>,
    cancel: CancellationToken,
    batch_history_multiplier: u8,
) {
    let mut buf = vec![0u8; 2048];
    let mut sweep_interval = tokio::time::interval(Duration::from_millis(200));
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
                            stream, peer, acceptor, sessions, require_auth, hash, encrypt_udp, key_timeout_ms, child_cancel, screen_width, screen_height, batch_history_multiplier,
                        ));
                    }
                    Err(e) => {
                        eprintln!("[spud] tcp accept: {e}");
                    }
                }
            }
            _ = sweep_interval.tick() => {
                for mut session in sessions.iter_mut() {
                    let actions = session.tracker.sweep();
                    for action in &actions {
                        println!("[server] (timeout): {action}");
                    }
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    if let Some(inj) = injector.get() {
                        for action in &actions {
                            inj.inject_action(action);
                        }
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
                                let batches = crate::net::Event::decode_all_batches(&pt);
                                if !batches.is_empty() {
                                    // Process redundant batches in ascending order (oldest first).
                                    // Wire order is: [current][newest_redundant]...[oldest_redundant],
                                    // so redundant batches are batches[1..] with newest at index 1.
                                    // Ascending = oldest first = iterate in reverse.
                                    let is_localhost = src.ip().is_loopback();
                                    for batch in batches[1..].iter().rev() {
                                        if session.mouse_history.contains(batch.seq_base) {
                                            continue;
                                        }
                                        for event in &batch.events {
                                            #[cfg(any(target_os = "linux", target_os = "macos"))]
                                            if let Some(inj) = injector.get() {
                                                if !is_localhost {
                                                    match event {
                                                        crate::net::Event::MouseMove { dx, dy } => {
                                                            inj.move_rel(i32::from(*dx), i32::from(*dy));
                                                        }
                                                        crate::net::Event::MouseAbs { x, y } => {
                                                            let px = (*x as i32 * (i32::from(session.screen_width) - 1) + 32767) / 65535;
                                                            let py = (*y as i32 * (i32::from(session.screen_height) - 1) + 32767) / 65535;
                                                            inj.move_abs(px, py);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        session.mouse_history.push(batch.seq_base);
                                    }

                                    // Process primary (current) batch fully.
                                    let primary = &batches[0];
                                    for event in &primary.events {
                                        // Deduplicate keyboard and wheel events by seq number.
                                        // Seq 0 is from old clients (backward compat) and bypasses dedup.
                                        let seq = match event {
                                            crate::net::Event::KeyDown(_, s) | crate::net::Event::KeyUp(_, s) | crate::net::Event::KeyRepeat(_, s) => Some(*s),
                                            crate::net::Event::Wheel { seq, .. } => Some(*seq),
                                            _ => None,
                                        };
                                        if let Some(s) = seq {
                                            if s != 0 && session.key_history.contains(s) {
                                                continue; // duplicate
                                            }
                                            if s != 0 {
                                                session.key_history.push(s);
                                            }
                                        }

                                        // If a repeat arrives without a prior down (lost packet),
                                        // inject the synthetic down before handling the repeat.
                                        let needs_key_down = matches!(
                                            event,
                                            crate::net::Event::KeyRepeat(c, _) if !session.tracker.has_key(*c)
                                        );
                                        let needs_button_down = matches!(
                                            event,
                                            crate::net::Event::MouseButtonRepeat(b) if !session.tracker.has_button(*b)
                                        );

                                        let actions = session.tracker.handle_event(event);
                                        if actions.is_empty() {
                                            println!("[server] {src}: {event:?}");
                                        } else {
                                            for action in &actions {
                                                println!("[server] {src}: {action}");
                                            }
                                        }
                                        #[cfg(any(target_os = "linux", target_os = "macos"))]
                                        if let Some(inj) = injector.get() {
                                            if !is_localhost {
                                                if needs_key_down {
                                                    if let crate::net::Event::KeyRepeat(code, _) = event {
                                                        inj.key_down(*code);
                                                    }
                                                }
                                                if needs_button_down {
                                                    if let crate::net::Event::MouseButtonRepeat(button) = event {
                                                        inj.button_down(wire_to_platform_button(*button));
                                                    }
                                                }
                                                match event {
                                                    crate::net::Event::KeyDown(code, _) => {
                                                        inj.key_down(*code);
                                                    }
                                                    crate::net::Event::KeyUp(code, _) => {
                                                        inj.key_up(*code);
                                                    }
                                                    crate::net::Event::KeyRepeat(_, _) => {
                                                        // Heartbeat - tracker already updated, no injection needed
                                                    }
                                                    crate::net::Event::MouseButton { button, pressed: true } => {
                                                        inj.button_down(wire_to_platform_button(*button));
                                                    }
                                                    crate::net::Event::MouseButton { button, pressed: false } => {
                                                        inj.button_up(wire_to_platform_button(*button));
                                                    }
                                                    crate::net::Event::MouseButtonRepeat(_) => {}
                                                    crate::net::Event::Wheel { dx, dy, .. } => {
                                                        inj.wheel(*dx, *dy);
                                                    }
                                                    crate::net::Event::MouseAbs { x, y } => {
                                                        let px = (*x as i32 * (i32::from(session.screen_width) - 1) + 32767) / 65535;
                                                        let py = (*y as i32 * (i32::from(session.screen_height) - 1) + 32767) / 65535;
                                                        inj.move_abs(px, py);
                                                    }
                                                    crate::net::Event::MouseMove { dx, dy } => {
                                                        println!("[server] MouseMove dx={dx} dy={dy} window_mode={}", session.window_mode);
                                                        inj.move_rel(i32::from(*dx), i32::from(*dy));
                                                    }
                                                    crate::net::Event::Keepalive => {}
                                                }
                                            }
                                        }
                                    }
                                    // Track the primary batch so future redundant copies are skipped.
                                    // Only mouse batches are sent redundantly; non-mouse primaries use seq_base=0
                                    // and are not redundantly transmitted, so skip those.
                                    let is_mouse_batch = primary.events.iter().any(|e| {
                                        matches!(e, crate::net::Event::MouseMove { .. } | crate::net::Event::MouseAbs { .. })
                                    });
                                    if is_mouse_batch {
                                        session.mouse_history.push(primary.seq_base);
                                    }
                                }
                            } else if session.encrypt {
                                should_remove = session.record_decrypt_failure();
                                if should_remove {
                                    eprintln!("[spud] UDP too many failed decrypts for conn {conn_id}, removing session");
                                }
                            }
                        }
                        else {
                            eprintln!("[spud] UDP event for unknown session {conn_id}");
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
    screen_width: u16,
    screen_height: u16,
    batch_history_multiplier: u8,
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
        let salt = crate::config::extract_salt(&passphrase_hash)
            .and_then(|s| crate::config::decode_salt_bytes(&s))
            .unwrap_or([0u8; 16]);
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
    let session = SessionState::new(encrypt_udp, keys, peer, key_timeout_ms, screen_width, screen_height);
    sessions.insert(conn_id, session);

    let init = ControlMsg::SessionInit { conn_id, uuid, encrypt: encrypt_udp, auth: require_auth && !passphrase_hash.is_empty(), screen_width, screen_height };
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
                        if let Ok(msg) = postcard::from_bytes::<ControlMsg>(&bytes) {
                            match msg {
                                ControlMsg::Keepalive => {
                                    if let Some(mut s) = sessions.get_mut(&conn_id) {
                                        s.last_activity = std::time::Instant::now();
                                    }
                                }
                                ControlMsg::SetCaptureMode { window_mode } => {
                                    if let Some(mut s) = sessions.get_mut(&conn_id) {
                                        s.window_mode = window_mode;
                                        println!("[server] conn {conn_id} capture mode: {}", if window_mode { "window" } else { "fullscreen" });
                                    }
                                }
                                ControlMsg::SetBatchConfig { max_batch, batch_redundancy } => {
                                    if let Some(mut s) = sessions.get_mut(&conn_id) {
                                        let capacity = max_batch as usize * batch_redundancy as usize * batch_history_multiplier as usize;
                                        s.mouse_history.resize(capacity);
                                        println!("[server] conn {conn_id} batch config: max_batch={max_batch} redundancy={batch_redundancy} history_capacity={capacity}");
                                    }
                                }
                                _ => {}
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

