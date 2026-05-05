use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc as ifmpsc;
use iced::futures::stream::Stream;

pub const PROTOCOL_VERSION: u16 = 1;
pub const FEATURES: u32 = 0;

#[derive(Debug, Clone)]
pub enum NetEvent {
    Disconnected,
}

fn event_sink() -> &'static Mutex<Option<ifmpsc::Sender<NetEvent>>> {
    static SINK: OnceLock<Mutex<Option<ifmpsc::Sender<NetEvent>>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(None))
}

pub fn events() -> impl Stream<Item = NetEvent> + 'static {
    iced::stream::channel(8, |tx: ifmpsc::Sender<NetEvent>| async move {
        *event_sink().lock().unwrap() = Some(tx);
        std::future::pending::<()>().await;
    })
}

fn push_event(event: NetEvent) {
    if let Some(tx) = event_sink().lock().unwrap().as_mut() {
        let _ = tx.try_send(event);
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    KeyDown(String),
    KeyUp(String),
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, pressed: bool },
    Wheel { dx: i8, dy: i8 },
    KeyRepeat(String),
    Heartbeat,
}

const TAG_KEY_DOWN: u8 = 0x01;
const TAG_KEY_UP: u8 = 0x02;
const TAG_MOUSE_MOVE: u8 = 0x03;
const TAG_MOUSE_BUTTON: u8 = 0x04;
const TAG_WHEEL: u8 = 0x05;
const TAG_KEY_REPEAT: u8 = 0x06;
const TAG_HEARTBEAT: u8 = 0x07;

const CTRL_HELLO: u8 = 0x01;
const CTRL_HELLO_ACK: u8 = 0x02;
const CTRL_AUTH_FAILED: u8 = 0x03;
const CTRL_AUTH: u8 = 0x04;
const CTRL_AUTH_ACK: u8 = 0x05;

impl Event {
    pub fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Event::KeyDown(name) => {
                buf.push(TAG_KEY_DOWN);
                push_name(buf, name);
            }
            Event::KeyUp(name) => {
                buf.push(TAG_KEY_UP);
                push_name(buf, name);
            }
            Event::KeyRepeat(name) => {
                buf.push(TAG_KEY_REPEAT);
                push_name(buf, name);
            }
            Event::MouseMove { dx, dy } => {
                buf.push(TAG_MOUSE_MOVE);
                buf.extend_from_slice(&dx.to_le_bytes());
                buf.extend_from_slice(&dy.to_le_bytes());
            }
            Event::MouseButton { button, pressed } => {
                buf.push(TAG_MOUSE_BUTTON);
                buf.push(*button);
                buf.push(if *pressed { 1 } else { 0 });
            }
            Event::Wheel { dx, dy } => {
                buf.push(TAG_WHEEL);
                buf.push(*dx as u8);
                buf.push(*dy as u8);
            }
            Event::Heartbeat => {
                buf.push(TAG_HEARTBEAT);
            }
        }
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        let (&tag, rest) = buf.split_first()?;
        match tag {
            TAG_KEY_DOWN => Some(Event::KeyDown(read_name(rest)?)),
            TAG_KEY_UP => Some(Event::KeyUp(read_name(rest)?)),
            TAG_KEY_REPEAT => Some(Event::KeyRepeat(read_name(rest)?)),
            TAG_MOUSE_MOVE => {
                let dx = i16::from_le_bytes(rest.get(..2)?.try_into().ok()?);
                let dy = i16::from_le_bytes(rest.get(2..4)?.try_into().ok()?);
                Some(Event::MouseMove { dx, dy })
            }
            TAG_MOUSE_BUTTON => {
                let button = *rest.first()?;
                let pressed = *rest.get(1)? != 0;
                Some(Event::MouseButton { button, pressed })
            }
            TAG_WHEEL => {
                let dx = *rest.first()? as i8;
                let dy = *rest.get(1)? as i8;
                Some(Event::Wheel { dx, dy })
            }
            TAG_HEARTBEAT => Some(Event::Heartbeat),
            _ => None,
        }
    }
}

fn push_name(buf: &mut Vec<u8>, name: &str) {
    let bytes = name.as_bytes();
    let len = bytes.len().min(255);
    buf.push(len as u8);
    buf.extend_from_slice(&bytes[..len]);
}

fn read_name(rest: &[u8]) -> Option<String> {
    let (&len, rest) = rest.split_first()?;
    let bytes = rest.get(..len as usize)?;
    String::from_utf8(bytes.to_vec()).ok()
}

fn read_name_split(rest: &[u8]) -> Option<(String, &[u8])> {
    let (&len, rest) = rest.split_first()?;
    let bytes = rest.get(..len as usize)?;
    let tail = rest.get(len as usize..)?;
    String::from_utf8(bytes.to_vec()).ok().map(|s| (s, tail))
}

fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(payload)
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf)?;
    let len = u16::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

fn protocol_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

#[derive(Clone, Debug)]
pub struct Sender {
    udp_tx: mpsc::Sender<Vec<u8>>,
    shutdown: Arc<AtomicBool>,
    pub negotiated_version: u16,
    pub negotiated_features: u32,
    pub key_timeout_ms: u16,
    pub server_hash: String,
    pub client_hash: String,
}

impl Sender {
    pub fn connect(
        host: &str,
        port: u16,
        passphrase: Option<&str>,
        passphrase_changed: bool,
        require_auth: bool,
        stored_hash: &str,
    ) -> io::Result<Self> {
        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no addresses"))?;
        let mut tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        tcp.set_nodelay(true)?;
        tcp.set_read_timeout(Some(Duration::from_secs(5)))?;

        // 1. Send Hello
        let mut hello = Vec::with_capacity(7);
        hello.push(CTRL_HELLO);
        hello.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        hello.extend_from_slice(&FEATURES.to_le_bytes());
        write_frame(&mut tcp, &hello)?;

        // 2. Read HelloAck
        let payload = read_frame(&mut tcp)?;
        let (&tag, rest) = payload
            .split_first()
            .ok_or_else(|| protocol_err("empty ack"))?;
        if tag != CTRL_HELLO_ACK {
            return Err(protocol_err("expected HelloAck"));
        }
        let version_bytes: [u8; 2] = rest
            .get(0..2)
            .ok_or_else(|| protocol_err("short ack"))?
            .try_into()
            .unwrap();
        let feature_bytes: [u8; 4] = rest
            .get(2..6)
            .ok_or_else(|| protocol_err("short ack"))?
            .try_into()
            .unwrap();
        let key_timeout_bytes: [u8; 2] = rest
            .get(6..8)
            .unwrap_or(&[0xe8, 0x03])
            .try_into()
            .unwrap();
        let (server_hash, _) = read_name_split(&rest[8..])
            .ok_or_else(|| protocol_err("missing hash"))?;

        let negotiated_version = u16::from_le_bytes(version_bytes);
        let negotiated_features = u32::from_le_bytes(feature_bytes);
        let key_timeout_ms = u16::from_le_bytes(key_timeout_bytes);
        if negotiated_version == 0 {
            return Err(protocol_err("incompatible protocol version"));
        }

        if require_auth && server_hash.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Server does not require authentication",
            ));
        }

        // 3. Compute client hash
        let client_hash = if passphrase_changed {
            let pass = passphrase.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Passphrase required")
            })?;
            let salt = crate::config::extract_salt(&server_hash)
                .ok_or_else(|| protocol_err("invalid server hash"))?;
            crate::config::hash_passphrase_with_salt(pass, &salt)
                .ok_or_else(|| protocol_err("failed to hash passphrase"))?
        } else if require_auth && server_hash != stored_hash {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Server passphrase has changed",
            ));
        } else {
            stored_hash.to_string()
        };

        // 4. Send Auth
        let mut auth = Vec::with_capacity(64);
        auth.push(CTRL_AUTH);
        push_name(&mut auth, &client_hash);
        write_frame(&mut tcp, &auth)?;

        // 5. Read Auth response
        let response = read_frame(&mut tcp)?;
        let (&resp_tag, _) = response
            .split_first()
            .ok_or_else(|| protocol_err("empty auth response"))?;
        if resp_tag == CTRL_AUTH_FAILED {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Incorrect passphrase",
            ));
        }
        if resp_tag != CTRL_AUTH_ACK {
            return Err(protocol_err("expected AuthAck"));
        }

        tcp.set_read_timeout(Some(Duration::from_millis(200)))?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let s = shutdown.clone();
        thread::spawn(move || run_tcp_reader(tcp, s));

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(&addr)?;
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            while let Ok(buf) = rx.recv() {
                if let Err(e) = socket.send(&buf) {
                    eprintln!("[spud] udp send: {e}");
                }
            }
        });
        Ok(Self {
            udp_tx: tx,
            shutdown,
            negotiated_version,
            negotiated_features,
            key_timeout_ms,
            server_hash,
            client_hash,
        })
    }

    pub fn send(&self, event: &Event) {
        let mut buf = Vec::with_capacity(16);
        event.encode(&mut buf);
        let _ = self.udp_tx.send(buf);
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

fn run_tcp_reader(mut stream: TcpStream, shutdown: Arc<AtomicBool>) {
    let mut buf = [0u8; 64];
    while !shutdown.load(Ordering::Relaxed) {
        match stream.read(&mut buf) {
            Ok(0) => {
                push_event(NetEvent::Disconnected);
                return;
            }
            Ok(_) => continue,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                push_event(NetEvent::Disconnected);
                return;
            }
        }
    }
}

pub struct Listener {
    shutdown: Arc<AtomicBool>,
    udp_thread: Option<thread::JoinHandle<()>>,
    tcp_thread: Option<thread::JoinHandle<()>>,
}

impl Listener {
    pub fn bind(
        addr: &str,
        port: u16,
        key_timeout_ms: u16,
        require_auth: bool,
        passphrase_hash: String,
    ) -> io::Result<Self> {
        let udp = UdpSocket::bind((addr, port))?;
        udp.set_read_timeout(Some(Duration::from_millis(200)))?;
        let tcp = TcpListener::bind((addr, port))?;
        tcp.set_nonblocking(true)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let allowed: Arc<Mutex<HashSet<IpAddr>>> = Arc::new(Mutex::new(HashSet::new()));

        let s = shutdown.clone();
        let allowed_udp = allowed.clone();
        let udp_thread = thread::spawn(move || run_udp(udp, s, allowed_udp, key_timeout_ms));

        let s = shutdown.clone();
        let tcp_thread = thread::spawn(move || {
            run_tcp_accept(
                tcp,
                s,
                allowed,
                key_timeout_ms,
                require_auth,
                passphrase_hash,
            )
        });

        Ok(Self {
            shutdown,
            udp_thread: Some(udp_thread),
            tcp_thread: Some(tcp_thread),
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(t) = self.udp_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.tcp_thread.take() {
            let _ = t.join();
        }
    }
}

fn run_udp(
    socket: UdpSocket,
    shutdown: Arc<AtomicBool>,
    allowed: Arc<Mutex<HashSet<IpAddr>>>,
    key_timeout_ms: u16,
) {
    let mut buf = [0u8; 1024];
    let mut tracker = KeyTracker::new(key_timeout_ms);
    while !shutdown.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                if !allowed.lock().unwrap().contains(&src.ip()) {
                    continue;
                }
                if let Some(event) = Event::decode(&buf[..n]) {
                    for action in tracker.handle(&event) {
                        println!("[server] {src}: {action}");
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(e) => {
                eprintln!("[spud] udp recv: {e}");
                break;
            }
        }
        for action in tracker.sweep() {
            println!("[server] (timeout): {action}");
        }
    }
}

struct KeyTracker {
    pressed: HashMap<String, Instant>,
    timeout: Duration,
}

impl KeyTracker {
    fn new(key_timeout_ms: u16) -> Self {
        Self {
            pressed: HashMap::new(),
            timeout: Duration::from_millis(u64::from(key_timeout_ms)),
        }
    }

    fn handle(&mut self, event: &Event) -> Vec<String> {
        match event {
            Event::KeyDown(name) => {
                let mut actions = Vec::new();
                if self.pressed.contains_key(name) {
                    actions.push(format!("release {name} (lost up)"));
                }
                actions.push(format!("press {name}"));
                self.pressed.insert(name.clone(), Instant::now());
                actions
            }
            Event::KeyRepeat(name) => {
                if self.pressed.contains_key(name) {
                    self.pressed.insert(name.clone(), Instant::now());
                    vec![format!("repeat {name}")]
                } else {
                    self.pressed.insert(name.clone(), Instant::now());
                    vec![format!("press {name} (repeat without prior down)")]
                }
            }
            Event::KeyUp(name) => {
                if self.pressed.remove(name).is_some() {
                    vec![format!("release {name}")]
                } else {
                    Vec::new()
                }
            }
            Event::MouseMove { dx, dy } => vec![format!("mouse move ({dx}, {dy})")],
            Event::MouseButton { button, pressed } => {
                let verb = if *pressed { "press" } else { "release" };
                vec![format!("mouse {verb} button {button}")]
            }
            Event::Wheel { dx, dy } => vec![format!("wheel ({dx}, {dy})")],
            Event::Heartbeat => Vec::new(),
        }
    }

    fn sweep(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut expired = Vec::new();
        self.pressed.retain(|name, last| {
            if now.duration_since(*last) > self.timeout {
                expired.push(format!("release {name} (timeout)"));
                false
            } else {
                true
            }
        });
        expired
    }
}

fn run_tcp_accept(
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    allowed: Arc<Mutex<HashSet<IpAddr>>>,
    key_timeout_ms: u16,
    require_auth: bool,
    passphrase_hash: String,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let allowed = allowed.clone();
                let s = shutdown.clone();
                let hash = passphrase_hash.clone();
                thread::spawn(move || {
                    handle_control(
                        stream,
                        peer.ip(),
                        allowed,
                        key_timeout_ms,
                        s,
                        require_auth,
                        hash,
                    )
                });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("[spud] tcp accept: {e}");
                break;
            }
        }
    }
}

fn handle_control(
    mut stream: TcpStream,
    peer_ip: IpAddr,
    allowed: Arc<Mutex<HashSet<IpAddr>>>,
    key_timeout_ms: u16,
    shutdown: Arc<AtomicBool>,
    require_auth: bool,
    passphrase_hash: String,
) {
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("[spud] tcp nodelay: {e}");
        return;
    }
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
        eprintln!("[spud] tcp opt: {e}");
        return;
    }

    // 1. Read Hello
    let payload = match read_frame(&mut stream) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[spud] tcp hello: {e}");
            return;
        }
    };
    let Some((&tag, rest)) = payload.split_first() else {
        return;
    };
    if tag != CTRL_HELLO {
        return;
    }
    let Some(version_slice) = rest.get(..2) else {
        return;
    };
    let Some(feature_slice) = rest.get(2..6) else {
        return;
    };
    let client_version = u16::from_le_bytes(version_slice.try_into().unwrap());
    let client_features = u32::from_le_bytes(feature_slice.try_into().unwrap());
    let negotiated_version = client_version.min(PROTOCOL_VERSION);
    let negotiated_features = client_features & FEATURES;

    // 2. Send HelloAck with hash
    let mut ack = Vec::with_capacity(64);
    ack.push(CTRL_HELLO_ACK);
    ack.extend_from_slice(&negotiated_version.to_le_bytes());
    ack.extend_from_slice(&negotiated_features.to_le_bytes());
    ack.extend_from_slice(&key_timeout_ms.to_le_bytes());
    if require_auth {
        push_name(&mut ack, &passphrase_hash);
    } else {
        push_name(&mut ack, "");
    }
    if let Err(e) = write_frame(&mut stream, &ack) {
        eprintln!("[spud] tcp ack: {e}");
        return;
    }

    if negotiated_version == 0 {
        return;
    }

    // 3. Read Auth
    let auth_payload = match read_frame(&mut stream) {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some((&auth_tag, auth_rest)) = auth_payload.split_first() else {
        return;
    };
    if auth_tag != CTRL_AUTH {
        return;
    }
    let client_hash = match read_name(auth_rest) {
        Some(h) => h,
        None => return,
    };

    // 4. Verify
    if require_auth && client_hash != passphrase_hash {
        let fail = vec![CTRL_AUTH_FAILED];
        let _ = write_frame(&mut stream, &fail);
        return;
    }

    // 5. Send AuthAck and add to allowed
    let auth_ack = vec![CTRL_AUTH_ACK];
    if let Err(e) = write_frame(&mut stream, &auth_ack) {
        eprintln!("[spud] tcp auth ack: {e}");
        return;
    }

    allowed.lock().unwrap().insert(peer_ip);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut buf = [0u8; 64];
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => continue,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
    allowed.lock().unwrap().remove(&peer_ip);
}
