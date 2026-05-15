use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{checkbox, column, container, mouse_area, row, slider, text, text_input};
use iced::{Background, Border, Color, Element, Length, Padding, Point, Shadow, Vector};

use crate::components as ui;
use crate::config::{CaptureMode, ClientConfig};
use crate::discovery::{self, DiscoveredServer};
use crate::icons;
use crate::net::protocol::ControlMsg;
use crate::theme as mt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connection,
    Capture,
    Mouse,
    Security,
    Advanced,
}

impl Page {
    const ALL: [Page; 5] = [Page::Connection, Page::Capture, Page::Mouse, Page::Security, Page::Advanced];

    fn label(self) -> &'static str {
        match self {
            Page::Connection => "Connection",
            Page::Mouse => "Mouse",
            Page::Capture => "Capture",
            Page::Security => "Security",
            Page::Advanced => "Advanced",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Connection => icons::PLUG,
            Page::Mouse => icons::COMPUTER_MOUSE,
            Page::Capture => icons::KEYBOARD,
            Page::Security => icons::SHIELD_HALVED,
            Page::Advanced => icons::GEAR,
        }
    }
}

const CAPTURE_MODES: [CaptureMode; 2] = [
    CaptureMode::Fullscreen,
    CaptureMode::Window,
];


#[derive(Debug, Clone)]
pub enum Message {
    SelectPage(Page),
    HostChanged(String),
    PortChanged(String),
    Connect,
    ConnectSuccess(crate::net::Sender, Option<String>),
    ConnectFailed(String),
    Disconnect,
    SensitivityChanged(f32),
    NaturalScrollToggled(bool),
    CaptureModeChanged(CaptureMode),
    RequireAuthToggled(bool),
    PassphraseChanged(String),
    SelectDiscovered(usize),
    DiscoveryEvent(discovery::Event),
    OpenHotkeyDialog,
    CloseHotkeyDialog,
    ConfirmHotkey,
    HotkeyInput(Key, Modifiers),
    Capture(iced::Event),
    HotkeyEvent(crate::input::InputEvent),
    ConnectionLost,
    ReconnectSuccess(crate::net::Sender, u64),
    ReconnectFailed(u64),
    KeyRepeatTick,
    KeepaliveTick,
    KeyRepeatIntervalChanged(u16),
    KeepaliveIntervalChanged(u16),
    ReconnectTimeoutChanged(String),
    BlankScreenToggled(bool),
    ShowHotkeyOnBlankToggled(bool),
    EncryptUdpToggled(bool),
    MouseBatchSizeChanged(u8),
    BatchRedundancyChanged(u8),
    UdpDropPercentChanged(u8),
    WindowSizeChanged(iced::Size),
    FingerprintMismatch { host: String, port: u16, new_fingerprint: [u8; 32] },
    FingerprintDialogCancel,
    FingerprintDialogAllowOnce { host: String, port: u16, new_fingerprint: [u8; 32] },
    FingerprintDialogTrust { host: String, port: u16, new_fingerprint: [u8; 32] },
    FingerprintDialogToggleFingerprint,
}

pub struct State {
    page: Page,
    host: String,
    port: String,
    connected: bool,
    connecting: bool,
    sensitivity: f32,
    natural_scroll: bool,
    capture_mode: CaptureMode,
    hotkey: String,
    require_auth: bool,
    passphrase: String,
    pending_passphrase: String,
    passphrase_hash: String,
    discovered: Vec<DiscoveredServer>,
    selected_addrs: Vec<SocketAddr>,
    selected_fullname: Option<String>,
    reconnect_cancel: Option<Arc<AtomicBool>>,
    pub hotkey_dialog_open: bool,
    pending_hotkey: String,
    sender: Option<crate::net::Sender>,
    last_cursor: Option<Point>,
    last_error: Option<String>,
    pressed_keys: HashSet<u16>,
    pressed_mouse_buttons: HashSet<u8>,
    key_repeat_interval_ms: u16,
    reconnecting: bool,
    reconnect_generation: u64,
    keepalive_interval_ms: u16,
    reconnect_timeout_secs: String,
    blank_screen: bool,
    show_hotkey_on_blank: bool,
    mouse_batch_size: u8,
    batch_redundancy: u8,
    udp_drop_percent: u8,
    grabbed: bool,
    /// User's explicit intent to capture. Persists even if the backend
    /// loses the grab unexpectedly (e.g. COSMIC pointer constraint bugs).
    user_capturing: bool,
    encrypt_udp: bool,
    server_screen_size: Option<(u16, u16)>,
    window_size: Option<iced::Size>,
    fingerprint_dialog: Option<FingerprintDialogState>,
}

#[derive(Debug, Clone)]
pub struct FingerprintDialogState {
    host: String,
    port: u16,
    new_fingerprint: [u8; 32],
    show_fingerprint: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::from_config(&ClientConfig::default())
    }
}

impl State {
    pub fn from_config(cfg: &ClientConfig) -> Self {
        Self {
            page: Page::Connection,
            host: cfg.host.clone(),
            port: cfg.port.clone(),
            connected: false,
            connecting: false,
            sensitivity: cfg.sensitivity.parse().unwrap_or(1.0),
            natural_scroll: cfg.natural_scroll,
            capture_mode: cfg.capture_mode,
            hotkey: cfg.hotkey.clone(),
            require_auth: cfg.require_auth,
            passphrase: String::new(),
            pending_passphrase: String::new(),
            passphrase_hash: cfg.passphrase_hash.clone(),
            discovered: Vec::new(),
            selected_addrs: Vec::new(),
            selected_fullname: None,
            reconnect_cancel: None,
            hotkey_dialog_open: false,
            pending_hotkey: String::new(),
            sender: None,
            last_cursor: None,
            last_error: None,
            pressed_keys: HashSet::new(),
            pressed_mouse_buttons: HashSet::new(),
            key_repeat_interval_ms: cfg.key_repeat_interval_ms,
            reconnecting: false,
            reconnect_generation: 0,
            keepalive_interval_ms: cfg.keepalive_interval_ms,
            reconnect_timeout_secs: cfg.reconnect_timeout_secs.to_string(),
            blank_screen: cfg.blank_screen,
            show_hotkey_on_blank: cfg.show_hotkey_on_blank,
            mouse_batch_size: cfg.mouse_batch_size,
            batch_redundancy: cfg.batch_redundancy,
            udp_drop_percent: cfg.udp_drop_percent,
            grabbed: false,
            user_capturing: false,
            encrypt_udp: cfg.encrypt_udp,
            server_screen_size: None,
            window_size: None,
            fingerprint_dialog: None,
        }
    }

    pub fn to_config(&self) -> ClientConfig {
        ClientConfig {
            host: self.host.clone(),
            port: self.port.clone(),
            sensitivity: format!("{:.2}", self.sensitivity),
            natural_scroll: self.natural_scroll,
            capture_mode: self.capture_mode,
            hotkey: self.hotkey.clone(),
            require_auth: self.require_auth,
            passphrase_hash: self.passphrase_hash.clone(),
            key_repeat_interval_ms: self.key_repeat_interval_ms,
            keepalive_interval_ms: self.keepalive_interval_ms,
            reconnect_timeout_secs: self.reconnect_timeout_secs.parse().unwrap_or(30),
            blank_screen: self.blank_screen,
            show_hotkey_on_blank: self.show_hotkey_on_blank,
            mouse_batch_size: self.mouse_batch_size,
            batch_redundancy: self.batch_redundancy,
            udp_drop_percent: self.udp_drop_percent,
            encrypt_udp: self.encrypt_udp,
        }
    }
}

impl State {
    /// If the current host/port input matches a discovered server by IP,
    /// select it. Otherwise clear the selection.
    fn sync_selection_from_input(&mut self) {
        if let Ok(ip) = self.host.parse::<std::net::IpAddr>() {
            if let Some(server) = self.discovered.iter().find(|s| {
                s.port == self.port && s.addrs.iter().any(|a| a.ip() == ip)
            }) {
                self.selected_fullname = Some(server.fullname.clone());
                self.selected_addrs = server.addrs.clone();
                return;
            }
        }
        self.selected_fullname = None;
        self.selected_addrs.clear();
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectPage(p) => {
                if self.page == Page::Security && p != Page::Security {
                    if !self.pending_passphrase.is_empty() {
                        self.passphrase = self.pending_passphrase.clone();
                    }
                    self.pending_passphrase.clear();
                }
                self.page = p;
            }
            Message::HostChanged(s) => {
                self.host = s;
                self.sync_selection_from_input();
            }
            Message::PortChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.port = s;
                    self.sync_selection_from_input();
                }
            }
            Message::Connect => {
                self.last_error = None;
                self.connecting = true;
            }
            Message::ConnectSuccess(sender, phc) => {
                if let Some(phc) = phc {
                    self.passphrase_hash = phc;
                }
                self.server_screen_size = sender.screen_size;
                if let Some((w, h)) = self.server_screen_size {
                    println!("[client] Server screen size: {w}x{h}");
                }
                let scale = self.mouse_scale();
                println!("[client] Mouse scale: x={:.2}, y={:.2}", scale.0, scale.1);
                let mode = self.capture_mode == CaptureMode::Window;
                println!("[client] sending SetCaptureMode window_mode={mode}");
                sender.send_control(ControlMsg::SetCaptureMode {
                    window_mode: mode,
                });
                sender.send_control(ControlMsg::SetBatchConfig {
                    max_batch: self.mouse_batch_size,
                    batch_redundancy: self.batch_redundancy,
                });
                self.sender = Some(sender);
                self.connected = true;
                self.connecting = false;
            }
            Message::ConnectFailed(e) => {
                self.last_error = Some(e);
                self.connecting = false;
            }
            Message::FingerprintMismatch { host, port, new_fingerprint } => {
                self.connecting = false;
                self.fingerprint_dialog = Some(FingerprintDialogState {
                    host,
                    port,
                    new_fingerprint,
                    show_fingerprint: false,
                });
            }
            Message::FingerprintDialogToggleFingerprint => {
                if let Some(ref mut d) = self.fingerprint_dialog {
                    d.show_fingerprint = !d.show_fingerprint;
                }
            }
            Message::FingerprintDialogCancel => {
                self.fingerprint_dialog = None;
            }
            Message::FingerprintDialogAllowOnce { .. } => {
                self.fingerprint_dialog = None;
                self.connecting = true;
                self.last_error = None;
            }
            Message::FingerprintDialogTrust { .. } => {
                self.fingerprint_dialog = None;
                self.connecting = true;
                self.last_error = None;
            }
            Message::Disconnect => {
                self.connected = false;
                self.connecting = false;
                self.sender = None;
                self.last_cursor = None;
                self.last_error = None;
                self.pressed_keys.clear();
                self.pressed_mouse_buttons.clear();
                self.reconnecting = false;
                self.reconnect_generation += 1;
                if let Some(cancel) = self.reconnect_cancel.take() {
                    cancel.store(true, Ordering::Relaxed);
                }
                self.grabbed = false;
                self.user_capturing = false;
            }
            Message::ConnectionLost => {
                if self.connected {
                    self.connected = false;
                    self.connecting = false;
                    self.sender = None;
                    self.last_cursor = None;
                    self.pressed_keys.clear();
                    self.pressed_mouse_buttons.clear();
                    self.reconnecting = true;
                    self.reconnect_generation += 1;
                    self.reconnect_cancel = None;
                    self.grabbed = false;
                    self.user_capturing = false;
                }
            }
            Message::ReconnectSuccess(sender, gen) => {
                if self.reconnecting && self.reconnect_generation == gen {
                    self.server_screen_size = sender.screen_size;
                    if let Some((w, h)) = self.server_screen_size {
                        println!("[client] Server screen size: {w}x{h}");
                    }
                    let scale = self.mouse_scale();
                    println!("[client] Mouse scale: x={:.2}, y={:.2}", scale.0, scale.1);
                    sender.send_control(ControlMsg::SetCaptureMode {
                        window_mode: self.capture_mode == CaptureMode::Window,
                    });
                    sender.send_control(ControlMsg::SetBatchConfig {
                        max_batch: self.mouse_batch_size,
                        batch_redundancy: self.batch_redundancy,
                    });
                    self.sender = Some(sender);
                    self.connected = true;
                    self.reconnecting = false;
                    self.reconnect_cancel = None;
                    self.grabbed = false;
                    self.user_capturing = false;
                    self.last_error = None;
                }
            }
            Message::ReconnectFailed(gen) => {
                if self.reconnecting && self.reconnect_generation == gen {
                    self.reconnecting = false;
                    self.reconnect_cancel = None;
                    self.grabbed = false;
                    self.user_capturing = false;
                    self.last_error = Some("Server closed the connection.".to_string());
                }
            }
            Message::SensitivityChanged(v) => {
                self.sensitivity = v;
                if self.connected {
                    let scale = self.mouse_scale();
                    println!("[client] Mouse scale: x={:.2}, y={:.2}", scale.0, scale.1);
                }
            }
            Message::NaturalScrollToggled(v) => self.natural_scroll = v,
            Message::WindowSizeChanged(size) => {
                self.window_size = Some(size);
                if self.connected {
                    let scale = self.mouse_scale();
                    println!("[client] Mouse scale: x={:.2}, y={:.2}", scale.0, scale.1);
                }
            }
            Message::CaptureModeChanged(m) => {
                if self.capture_mode == CaptureMode::Fullscreen && m != CaptureMode::Fullscreen {
                    if crate::input::is_wayland_grabbed() {
                        crate::input::toggle_wayland_grab();
                    }
                }
                self.grabbed = false;
                self.release_all_held();
                self.capture_mode = m;
                if let Some(ref sender) = self.sender {
                    let mode = m == CaptureMode::Window;
                    println!("[client] sending SetCaptureMode window_mode={mode}");
                    sender.send_control(ControlMsg::SetCaptureMode {
                        window_mode: mode,
                    });
                }
                if self.connected {
                    let scale = self.mouse_scale();
                    println!("[client] Mouse scale: x={:.2}, y={:.2}", scale.0, scale.1);
                }
            }
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.pending_passphrase = s,
            Message::SelectDiscovered(i) => {
                if let Some(server) = self.discovered.get(i) {
                    // Use the resolved IP for connection instead of hostname.
                    self.host = server
                        .addrs
                        .first()
                        .map(|a| a.ip().to_string())
                        .unwrap_or_else(|| server.host.clone());
                    self.port = server.port.clone();
                    self.selected_addrs = server.addrs.clone();
                    self.selected_fullname = Some(server.fullname.clone());
                }
            }
            Message::DiscoveryEvent(event) => {
                match event {
                    discovery::Event::Found(server) => {
                        self.discovered.retain(|s| s.fullname != server.fullname);
                        self.discovered.push(server);
                        self.discovered.sort_by(|a, b| a.name.cmp(&b.name));
                    }
                    discovery::Event::Lost(fullname) => {
                        self.discovered.retain(|s| s.fullname != fullname);
                    }
                }
                self.sync_selection_from_input();
            }
            Message::OpenHotkeyDialog => {
                self.hotkey_dialog_open = true;
                self.pending_hotkey = String::new();
            }
            Message::CloseHotkeyDialog => {
                self.hotkey_dialog_open = false;
                self.pending_hotkey = String::new();
            }
            Message::ConfirmHotkey => {
                if !self.pending_hotkey.is_empty() {
                    self.hotkey = self.pending_hotkey.clone();
                }
                self.hotkey_dialog_open = false;
                self.pending_hotkey = String::new();
            }
            Message::HotkeyInput(key, mods) => {
                use iced::keyboard::key::Named;
                if matches!(key, Key::Named(Named::Escape)) {
                    self.hotkey_dialog_open = false;
                    self.pending_hotkey = String::new();
                } else if let Some(chord) = format_chord(&key, mods) {
                    self.pending_hotkey = chord;
                }
            }
            Message::Capture(event) => {
                if matches!(event, iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })) {
                    println!("[client] Capture CursorMoved");
                }
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) = &event
                {
                    if format_chord(key, *modifiers).as_deref() == Some(self.hotkey.as_str())
                    {
                        if self.capture_mode == CaptureMode::Fullscreen {
                            let new_grab = crate::input::toggle_wayland_grab();
                            self.user_capturing = new_grab;
                            self.grabbed = new_grab;
                        } else {
                            self.user_capturing = !self.user_capturing;
                            self.grabbed = self.user_capturing;
                            if !self.grabbed {
                                self.release_all_held();
                            }
                        }
                        return;
                    }
                }
                let forward = match self.capture_mode {
                    CaptureMode::Window => self.grabbed,
                    CaptureMode::Fullscreen => crate::input::is_wayland_grabbed(),
                };
                if forward {
                    let is_window_mode = matches!(self.capture_mode, CaptureMode::Window);
                    let scale = if is_window_mode {
                        (1.0, 1.0)
                    } else {
                        (self.sensitivity, self.sensitivity)
                    };
                    if let Some(wire) = iced_to_wire(
                        &event,
                        &mut self.last_cursor,
                        &mut self.pressed_keys,
                        &mut self.pressed_mouse_buttons,
                        scale,
                        self.window_size,
                        is_window_mode,
                        self.natural_scroll,
                    ) {
                        if matches!(wire, crate::net::Event::MouseMove { .. } | crate::net::Event::MouseAbs { .. }) {
                            println!("[client] send {:?}", wire);
                        }
                        if let Some(sender) = &self.sender {
                            sender.send(&wire);
                        }
                    }
                }
            }
            Message::HotkeyEvent(event) => {
                if let crate::input::InputEvent::HotkeyToggled { grabbed } = event {
                    self.grabbed = grabbed;
                    self.user_capturing = grabbed;
                    if !grabbed {
                        self.release_all_held();
                    }
                    return;
                }
                if let crate::input::InputEvent::BackendError(ref msg) = event {
                    eprintln!("[client] input backend error: {msg}");
                }
                if let Some(wire) = input_event_to_wire(&event, &mut self.pressed_keys, &mut self.pressed_mouse_buttons, self.sensitivity, self.natural_scroll) {
                    if let Some(sender) = &self.sender {
                        sender.send(&wire);
                    }
                }
            }
            Message::KeyRepeatTick => {
                if let Some(sender) = &self.sender {
                    for code in &self.pressed_keys {
                        sender.send(&crate::net::Event::KeyRepeat(*code));
                    }
                    for button in &self.pressed_mouse_buttons {
                        sender.send(&crate::net::Event::MouseButtonRepeat(*button));
                    }
                }
            }
            Message::KeepaliveTick => {
                if let Some(sender) = &self.sender {
                    sender.send(&crate::net::Event::Keepalive);
                }
            }
            Message::KeepaliveIntervalChanged(v) => {
                self.keepalive_interval_ms = (v / 10) * 10;
            }
            Message::ReconnectTimeoutChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.reconnect_timeout_secs = s;
                }
            }
            Message::BlankScreenToggled(v) => self.blank_screen = v,
            Message::ShowHotkeyOnBlankToggled(v) => self.show_hotkey_on_blank = v,
            Message::EncryptUdpToggled(v) => self.encrypt_udp = v,
            Message::MouseBatchSizeChanged(v) => {
                self.mouse_batch_size = v;
                if let Some(sender) = &self.sender {
                    sender.set_mouse_batch_size(v);
                    sender.send_control(ControlMsg::SetBatchConfig {
                        max_batch: self.mouse_batch_size,
                        batch_redundancy: self.batch_redundancy,
                    });
                }
            }
            Message::BatchRedundancyChanged(v) => {
                self.batch_redundancy = v;
                if let Some(sender) = &self.sender {
                    sender.set_batch_redundancy(v);
                    sender.send_control(ControlMsg::SetBatchConfig {
                        max_batch: self.mouse_batch_size,
                        batch_redundancy: self.batch_redundancy,
                    });
                }
            }
            Message::KeyRepeatIntervalChanged(v) => self.key_repeat_interval_ms = v,
            Message::UdpDropPercentChanged(v) => {
                self.udp_drop_percent = v;
                if let Some(sender) = &self.sender {
                    sender.set_udp_drop_percent(v);
                }
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn is_capturing_window(&self) -> bool {
        self.connected && self.capture_mode == CaptureMode::Window
    }

    pub fn is_capturing_fullscreen(&self) -> bool {
        self.connected && self.capture_mode == CaptureMode::Fullscreen
    }

    pub fn keyrepeat_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.key_repeat_interval_ms))
    }

    pub fn keepalive_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.keepalive_interval_ms))
    }

    pub fn reconnect_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(u64::from(self.reconnect_timeout_secs.parse::<u16>().unwrap_or(30)))
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn is_blank_screen_active(&self) -> bool {
        self.connected && self.capture_mode == CaptureMode::Fullscreen && self.user_capturing && self.blank_screen
    }

    fn mouse_scale(&self) -> (f32, f32) {
        match self.capture_mode {
            CaptureMode::Fullscreen => (self.sensitivity, self.sensitivity),
            CaptureMode::Window => {
                let (sw, sh) = match self.server_screen_size {
                    Some((w, h)) => (w as f32, h as f32),
                    None => return (1.0, 1.0),
                };
                let (ww, wh) = match self.window_size {
                    Some(s) => (s.width, s.height),
                    None => return (1.0, 1.0),
                };
                if ww > 0.0 && wh > 0.0 {
                    (sw / ww, sh / wh)
                } else {
                    (1.0, 1.0)
                }
            }
        }
    }

    pub fn show_hotkey_on_blank(&self) -> bool {
        self.show_hotkey_on_blank
    }

    pub fn capture_mode(&self) -> CaptureMode {
        self.capture_mode
    }

    fn release_all_held(&mut self) {
        if let Some(sender) = &self.sender {
            for name in &self.pressed_keys {
                sender.send(&crate::net::Event::KeyUp(name.clone()));
            }
            for button in &self.pressed_mouse_buttons {
                sender.send(&crate::net::Event::MouseButton { button: *button, pressed: false });
            }
        }
        self.pressed_keys.clear();
        self.pressed_mouse_buttons.clear();
    }

    pub fn hotkey_display(&self) -> &str {
        &self.hotkey
    }

    pub fn require_auth(&self) -> bool {
        self.require_auth
    }

    pub fn connection_passphrase(&self) -> Option<&str> {
        if self.require_auth {
            Some(self.passphrase.as_str()).filter(|p| !p.is_empty())
        } else {
            None
        }
    }

    pub fn passphrase_hash(&self) -> &str {
        &self.passphrase_hash
    }

    pub fn is_reconnecting(&self) -> bool {
        self.reconnecting
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn selected_addrs(&self) -> &[SocketAddr] {
        &self.selected_addrs
    }

    pub fn set_reconnect_cancel(&mut self, cancel: Arc<AtomicBool>) {
        self.reconnect_cancel = Some(cancel);
    }

    pub fn port(&self) -> &str {
        &self.port
    }

    pub fn reconnect_generation(&self) -> u64 {
        self.reconnect_generation
    }

    pub fn hotkey_string(&self) -> &str {
        &self.hotkey
    }

    pub fn encrypt_udp(&self) -> bool {
        self.encrypt_udp
    }

    pub fn mouse_batch_size(&self) -> u8 {
        self.mouse_batch_size
    }

    pub fn batch_redundancy(&self) -> u8 {
        self.batch_redundancy
    }

    pub fn udp_drop_percent(&self) -> u8 {
        self.udp_drop_percent
    }

    pub fn nav_items(&self, about_active: bool) -> Vec<Element<'_, Message>> {
        Page::ALL
            .iter()
            .copied()
            .map(|p| {
                ui::nav_item(
                    p.label(),
                    p.icon(),
                    !about_active && p == self.page,
                    Message::SelectPage(p),
                )
            })
            .collect()
    }

    pub fn view_content(&self, content_width: f32, server_running: bool) -> Element<'_, Message> {
        match self.page {
            Page::Connection => self.connection_page(content_width, server_running),
            Page::Mouse => self.input_page(),
            Page::Capture => self.hotkeys_page(),
            Page::Security => self.security_page(),
            Page::Advanced => self.advanced_page(),
        }
    }

    fn connection_page(&self, content_width: f32, server_running: bool) -> Element<'_, Message> {
        let status_label = if self.reconnecting {
            "Reconnecting..."
        } else if self.connecting {
            "Connecting..."
        } else if self.connected && !self.require_auth {
            "Connected (insecure)"
        } else if self.connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let status_color = if self.connected {
            mt::SUCCESS
        } else if self.reconnecting || self.connecting {
            mt::WARNING
        } else {
            mt::ON_SURFACE_VARIANT
        };

        let status_row: Element<Message> = if self.connected {
            let (icon, accent) = if self.require_auth {
                (icons::LOCK, mt::SUCCESS)
            } else {
                (icons::TRIANGLE_EXCLAMATION, mt::DANGER)
            };
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(icon).font(icons::FA_SOLID).size(13).color(accent),
                text(status_label).size(14).color(accent),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else if self.reconnecting || self.connecting {
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(icons::ROTATE).font(icons::FA_SOLID).size(13).color(mt::WARNING),
                text(status_label).size(14).color(mt::WARNING),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(status_label).size(14).color(status_color),
            ]
            .spacing(8)
            .into()
        };

        let mut host_input = text_input("e.g. 192.168.1.42 or hostname.local", &self.host)
            .padding(12)
            .size(14);
        let is_active = self.connected || self.reconnecting || self.connecting;
        if !is_active {
            host_input = host_input.on_input(Message::HostChanged);
        }
        let host_field: Element<Message> = if !is_active && self.host.is_empty() {
            column![
                ui::field_label("Server address"),
                host_input,
                text("Server address is required.").size(12).color(mt::DANGER),
            ]
            .spacing(6)
            .into()
        } else {
            column![ui::field_label("Server address"), host_input].spacing(6).into()
        };

        let mut port_input = text_input("7878", &self.port)
            .padding(12)
            .size(14)
            .width(Length::Fixed(120.0));
        if !is_active {
            port_input = port_input.on_input(Message::PortChanged);
        }
        let port_out_of_range = !self.port.is_empty()
            && !self.port.parse::<u16>().is_ok_and(|p| p > 0);

        let port_field: Element<Message> = if !is_active && self.port.is_empty() {
            column![
                ui::field_label("Port"),
                port_input,
                text("Port is required.").size(12).color(mt::DANGER),
            ]
            .spacing(6)
            .into()
        } else if port_out_of_range {
            column![
                ui::field_label("Port"),
                port_input,
                text("Port must be between 1 and 65535.").size(12).color(mt::DANGER),
            ]
            .spacing(6)
            .into()
        } else {
            column![ui::field_label("Port"), port_input].spacing(6).into()
        };

        let can_connect = !self.host.is_empty()
            && self.port.parse::<u16>().is_ok_and(|p| p > 0);

        let action: Element<Message> = if is_active {
            ui::outlined_button("Disconnect", Some(Message::Disconnect))
        } else {
            ui::filled_button("Connect", (can_connect && !server_running).then_some(Message::Connect))
        };

        let mut conn_items: Vec<Element<Message>> = vec![
            status_row.into(),
            ui::v_space(16.0).into(),
            host_field,
            ui::v_space(12.0).into(),
            port_field,
        ];

        if server_running && !is_active {
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(ui::divider().into());
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::WARNING),
                    text("Stop the server before connecting as a client.")
                        .size(13)
                        .color(mt::WARNING),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        if self.udp_drop_percent > 0 {
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(ui::divider().into());
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::WARNING),
                    text(format!("Dropping {}% of UDP packets for testing.", self.udp_drop_percent))
                        .size(13)
                        .color(mt::WARNING),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        if let Some(err) = &self.last_error {
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(ui::divider().into());
            conn_items.push(ui::v_space(12.0).into());
            conn_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::DANGER),
                    text(err.as_str()).size(13).color(mt::DANGER),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        conn_items.push(ui::v_space(20.0).into());
        conn_items.push(row![ui::h_space_fill(), action].width(Length::Fill).into());

        let mut conn_items_with_title: Vec<Element<Message>> = vec![
            ui::card_title("Connection").into(),
            ui::v_space(12.0).into(),
        ];
        conn_items_with_title.extend(conn_items);
        let connection_card = ui::card(column(conn_items_with_title).spacing(0));

        let discovery_card = self.discovery_card(content_width);

        let body = column![discovery_card, ui::v_space(16.0), connection_card].spacing(0);
        ui::page_body("Connection", body)
    }

    pub fn discovery_card(&self, available_width: f32) -> Element<'_, Message> {
        let tile_width = 150.0_f32;
        let spacing = 12.0_f32;
        let cols = (((available_width + spacing) / (tile_width + spacing)).floor() as usize).max(1);

        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        for (chunk_idx, chunk) in self.discovered.chunks(cols).enumerate() {
            let base = chunk_idx * cols;
            let cells: Vec<Element<Message>> = chunk
                .iter()
                .enumerate()
                .map(|(j, s)| {
                    let idx = base + j;
                    let selected = self.selected_fullname.as_ref() == Some(&s.fullname);
                    let on_press = (!self.connected && !self.connecting && !self.reconnecting)
                        .then_some(Message::SelectDiscovered(idx));
                    let ip_address = s
                        .addrs
                        .first()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| s.address.clone());
                    ui::server_tile(
                        s.icon,
                        s.name.as_str(),
                        s.host.as_str(),
                        ip_address,
                        s.auth,
                        s.encrypt,
                        selected,
                        on_press,
                    )
                })
                .collect();
            grid_rows.push(row(cells).spacing(spacing).into());
        }
        let grid = column(grid_rows).spacing(spacing);

        ui::card(
            column![
                ui::card_title("Discovered servers"),
                ui::v_space(4.0),
                ui::helper_text("Tap a server to fill in the connection details."),
                ui::v_space(16.0),
                grid,
            ]
            .spacing(0),
        )
    }

    fn input_page(&self) -> Element<'_, Message> {
        let sens_card = ui::card(
            column![
                ui::card_title("Mouse sensitivity"),
                ui::v_space(4.0),
                ui::helper_text(
                    "Multiplier applied to relative mouse movement before sending."
                ),
                ui::v_space(16.0),
                row![
                    slider(0.25..=3.0, self.sensitivity, Message::SensitivityChanged)
                        .step(0.05)
                        .width(Length::Fill),
                    ui::h_space(16.0),
                    text(format!("{:.2}x", self.sensitivity))
                        .size(14)
                        .color(mt::ON_SURFACE),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        );

        let scroll_card = ui::card(
            column![
                ui::card_title("Scrolling"),
                ui::v_space(12.0),
                row![
                    column![
                        text("Natural scroll direction").size(16).color(mt::ON_SURFACE),
                        ui::v_space(2.0),
                        ui::helper_text("Invert the scroll direction sent to the server."),
                    ]
                    .width(Length::Fill),
                    checkbox(self.natural_scroll).on_toggle(Message::NaturalScrollToggled),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        );

        let body = if self.capture_mode == CaptureMode::Window {
            column![scroll_card].spacing(0)
        } else {
            column![sens_card, ui::v_space(16.0), scroll_card].spacing(0)
        };
        ui::page_body("Input", body)
    }

    fn hotkeys_page(&self) -> Element<'_, Message> {
        let capture_card = ui::card(
            column![
                ui::card_title("Capture mode"),
                ui::v_space(4.0),
                ui::helper_text("Decide when input is captured and forwarded."),
                ui::v_space(16.0),
                ui::pick_list(
                    CAPTURE_MODES,
                    Some(self.capture_mode),
                    Message::CaptureModeChanged,
                )
            ]
            .spacing(0),
        );

        let mut body_items: Vec<Element<Message>> = vec![capture_card.into()];

        let hotkey_card = ui::card(
            column![
                ui::card_title("Capture hotkey"),
                ui::v_space(4.0),
                ui::helper_text("Press this combo to toggle input capture."),
                ui::v_space(16.0),
                row![
                    text(&self.hotkey).size(14).color(mt::ON_SURFACE),
                    ui::h_space_fill(),
                    ui::outlined_button("Record hotkey", Some(Message::OpenHotkeyDialog)),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        );

        let show_hotkey_row = row![
            column![
                text("Show hotkey on blank screen").size(16).color(mt::ON_SURFACE),
                ui::v_space(2.0),
                ui::helper_text("Display the exit combo on the black overlay."),
            ]
            .width(Length::Fill),
            checkbox(self.show_hotkey_on_blank).on_toggle(Message::ShowHotkeyOnBlankToggled),
        ]
        .align_y(iced::Alignment::Center);

        let blank_card = if self.capture_mode == CaptureMode::Fullscreen {
            let blank_screen_row = row![
                column![
                    text("Blank while captured").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text("Show a black overlay while input is captured."),
                ]
                .width(Length::Fill),
                checkbox(self.blank_screen).on_toggle(Message::BlankScreenToggled),
            ]
            .align_y(iced::Alignment::Center);

            if self.blank_screen {
                ui::card(column![
                    ui::card_title("Blank screen"),
                    ui::v_space(12.0),
                    blank_screen_row,
                    ui::v_space(16.0),
                    show_hotkey_row,
                ].spacing(0))
            } else {
                ui::card(column![
                    ui::card_title("Blank screen"),
                    ui::v_space(12.0),
                    blank_screen_row,
                ].spacing(0))
            }
        } else {
            ui::card(column![
                ui::card_title("Blank screen"),
                ui::v_space(12.0),
                show_hotkey_row,
            ].spacing(0))
        };

        body_items.push(ui::v_space(16.0).into());
        body_items.push(hotkey_card.into());
        body_items.push(ui::v_space(16.0).into());
        body_items.push(blank_card.into());

        let body = column(body_items).spacing(0);
        ui::page_body("Capture", body)
    }

    pub fn hotkey_dialog(&self) -> Option<Element<'_, Message>> {
        if !self.hotkey_dialog_open {
            return None;
        }

        let chord_display: Element<Message> = if self.pending_hotkey.is_empty() {
            text("Hold your desired key combination...")
                .size(16)
                .color(mt::ON_SURFACE_VARIANT)
                .into()
        } else {
            text(&self.pending_hotkey)
                .size(22)
                .color(mt::ON_SURFACE)
                .into()
        };

        let dialog = container(
            column![
                text("Record hotkey").size(18).color(mt::ON_SURFACE),
                ui::v_space(6.0),
                ui::helper_text(
                    "Hold the key combination you want, then click 'Use this hotkey'.",
                ),
                ui::v_space(24.0),
                container(chord_display)
                    .width(Length::Fill)
                    .padding(Padding::from([20, 16]))
                    .style(|_| container::Style {
                        background: Some(Background::Color(mt::with_alpha(mt::PRIMARY, 0.06))),
                        border: Border {
                            color: mt::OUTLINE_VARIANT,
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    }),
                ui::v_space(24.0),
                row![
                    ui::h_space_fill(),
                    ui::outlined_button("Cancel", Some(Message::CloseHotkeyDialog)),
                    ui::h_space(8.0),
                    ui::filled_button(
                        "Use this hotkey",
                        (!self.pending_hotkey.is_empty()).then_some(Message::ConfirmHotkey),
                    ),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        )
        .width(Length::Fixed(440.0))
        .padding(Padding::from(28))
        .style(|_| container::Style {
            background: Some(Background::Color(mt::SURFACE)),
            border: Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            shadow: Shadow {
                color: mt::with_alpha(Color::BLACK, 0.3),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 32.0,
            },
            ..Default::default()
        });

        let backdrop = mouse_area(
            container(dialog)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(mt::with_alpha(Color::BLACK, 0.45))),
                    ..Default::default()
                }),
        )
        .interaction(iced::mouse::Interaction::Idle);

        Some(backdrop.into())
    }

    pub fn fingerprint_dialog(&self) -> Option<Element<'_, Message>> {
        let state = self.fingerprint_dialog.as_ref()?;
        let new_fp = hex::encode(&state.new_fingerprint);
        let host = state.host.clone();
        let port = state.port;

        // Format fingerprint into groups of 8 hex chars, 4 groups per line
        let formatted_fp: String = new_fp
            .as_bytes()
            .chunks(8)
            .enumerate()
            .map(|(i, chunk)| {
                let s = std::str::from_utf8(chunk).unwrap();
                if i > 0 && i % 4 == 0 {
                    format!("\n{s}")
                } else if i > 0 {
                    format!(" {s}")
                } else {
                    s.to_string()
                }
            })
            .collect();

        let mut content = column![
            row![
                text(icons::TRIANGLE_EXCLAMATION)
                    .font(icons::FA_SOLID)
                    .size(20)
                    .color(mt::WARNING),
                ui::h_space(8.0),
                text("Server identity changed").size(18).color(mt::ON_SURFACE),
            ]
            .align_y(iced::Alignment::Center),
            ui::v_space(6.0),
            text(format!(
                "The computer at {host}:{port} has a different security key than last time."
            ))
            .size(14)
            .color(mt::ON_SURFACE_VARIANT),
            ui::v_space(8.0),
            text("This usually means the server was reinstalled or updated. If you did not expect this, someone else may be trying to impersonate that computer.")
                .size(14)
                .color(mt::ON_SURFACE_VARIANT),
            ui::v_space(16.0),
            mouse_area(
                row![
                    text(if state.show_fingerprint {
                        icons::ANGLE_DOWN
                    } else {
                        icons::ANGLE_RIGHT
                    })
                    .font(icons::FA_SOLID)
                    .size(12)
                    .color(mt::ON_SURFACE_VARIANT),
                    ui::h_space(6.0),
                    text(if state.show_fingerprint {
                        "Hide nerd stuff"
                    } else {
                        "Show nerd stuff"
                    })
                    .size(12)
                    .color(mt::ON_SURFACE_VARIANT),
                ]
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::FingerprintDialogToggleFingerprint),
        ]
        .spacing(0);

        if state.show_fingerprint {
            let known = crate::config::load_known_servers();
            let key = format!("{host}:{port}");
            if let Some(known_fp) = known.get(&key) {
                let formatted_known: String = known_fp
                    .as_bytes()
                    .chunks(8)
                    .enumerate()
                    .map(|(i, chunk)| {
                        let s = std::str::from_utf8(chunk).unwrap();
                        if i > 0 && i % 4 == 0 {
                            format!("\n{s}")
                        } else if i > 0 {
                            format!(" {s}")
                        } else {
                            s.to_string()
                        }
                    })
                    .collect();
                content = content.push(ui::v_space(8.0)).push(
                    ui::helper_text("Known fingerprint:")
                ).push(ui::v_space(4.0)).push(
                    container(
                        text(formatted_known)
                            .size(12)
                            .font(iced::Font::MONOSPACE)
                            .color(mt::ON_SURFACE_VARIANT),
                    )
                    .width(Length::Fill)
                    .padding(Padding::from([12, 16]))
                    .style(|_| container::Style {
                        background: Some(Background::Color(mt::with_alpha(mt::PRIMARY, 0.06))),
                        border: Border {
                            color: mt::OUTLINE_VARIANT,
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    }),
                );
            }
            content = content.push(ui::v_space(8.0)).push(
                ui::helper_text("New fingerprint:")
            ).push(ui::v_space(4.0)).push(
                container(
                    text(formatted_fp)
                        .size(12)
                        .font(iced::Font::MONOSPACE)
                        .color(mt::ON_SURFACE_VARIANT),
                )
                .width(Length::Fill)
                .padding(Padding::from([12, 16]))
                .style(|_| container::Style {
                    background: Some(Background::Color(mt::with_alpha(mt::PRIMARY, 0.06))),
                    border: Border {
                        color: mt::OUTLINE_VARIANT,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }),
            );
        }

        content = content.push(ui::v_space(24.0)).push(
            row![
                ui::outlined_button("Don't connect", Some(Message::FingerprintDialogCancel)),
                ui::h_space(8.0),
                ui::outlined_button("Connect once", Some(Message::FingerprintDialogAllowOnce {
                    host: host.clone(),
                    port,
                    new_fingerprint: state.new_fingerprint,
                })),
                ui::h_space(8.0),
                ui::filled_button("Trust and connect", Some(Message::FingerprintDialogTrust {
                    host: host.clone(),
                    port,
                    new_fingerprint: state.new_fingerprint,
                })),
            ]
            .align_y(iced::Alignment::Center),
        );

        let dialog = container(content)
        .width(Length::Fixed(540.0))
        .padding(Padding::from(28))
        .style(|_| container::Style {
            background: Some(Background::Color(mt::SURFACE)),
            border: Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            shadow: Shadow {
                color: mt::with_alpha(Color::BLACK, 0.3),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 32.0,
            },
            ..Default::default()
        });

        let backdrop = mouse_area(
            container(dialog)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(mt::with_alpha(Color::BLACK, 0.45))),
                    ..Default::default()
                }),
        )
        .interaction(iced::mouse::Interaction::Idle);

        Some(backdrop.into())
    }

    fn security_page(&self) -> Element<'_, Message> {
        let mut auth_items: Vec<Element<Message>> = vec![
            row![
                column![
                    text("Use passphrase").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text("Send a passphrase when connecting to the server."),
                ]
                .width(Length::Fill),
                {
                    let cb = checkbox(self.require_auth);
                    if !self.connected {
                        cb.on_toggle(Message::RequireAuthToggled)
                    } else {
                        cb
                    }
                }
            ]
            .align_y(iced::Alignment::Center)
            .into(),
        ];

        if self.require_auth {
            auth_items.push(ui::v_space(16.0).into());
            auth_items.push(text("Passphrase").size(16).color(mt::ON_SURFACE).into());
            auth_items.push(ui::v_space(4.0).into());
            auth_items.push(ui::helper_text("Must match the passphrase set on the server.").into());
            auth_items.push(ui::v_space(16.0).into());
            {
                let mut input = text_input("Enter passphrase", &self.pending_passphrase)
                    .secure(true)
                    .padding(12)
                    .size(14);
                if !self.connected {
                    input = input.on_input(Message::PassphraseChanged);
                }
                auth_items.push(input.into());
            }

            if self.pending_passphrase.is_empty() {
                let has_passphrase = !self.passphrase.is_empty() || !self.passphrase_hash.is_empty();
                if has_passphrase {
                    auth_items.push(ui::v_space(8.0).into());
                    auth_items.push(
                        row![
                            text(icons::LOCK)
                                .font(icons::FA_SOLID)
                                .size(11)
                                .color(mt::SUCCESS),
                            text("Passphrase is set.")
                                .size(12)
                                .color(mt::SUCCESS),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center)
                        .into(),
                    );
                } else {
                    auth_items.push(ui::v_space(8.0).into());
                    auth_items.push(
                        row![
                            text(icons::TRIANGLE_EXCLAMATION)
                                .font(icons::FA_SOLID)
                                .size(11)
                                .color(mt::WARNING),
                            text("A passphrase is required when authentication is enabled.")
                                .size(12)
                                .color(mt::WARNING),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center)
                        .into(),
                    );
                }
            }
        }

        let mut auth_items_with_title: Vec<Element<Message>> = vec![
            ui::card_title("Authentication").into(),
            ui::v_space(12.0).into(),
        ];
        auth_items_with_title.extend(auth_items);
        let auth_card = ui::card(column(auth_items_with_title).spacing(0));

        let encrypt_card = ui::card(
            column![
                ui::card_title("Encryption"),
                ui::v_space(12.0),
                row![
                    column![
                        text("Require encryption").size(16).color(mt::ON_SURFACE),
                        ui::v_space(2.0),
                        ui::helper_text("Encrypt input events sent over the network. Disabling this is less secure, but reduces latency."),
                    ]
                    .width(Length::Fill),
                    {
                        let cb = checkbox(self.encrypt_udp);
                        if !self.connected {
                            cb.on_toggle(Message::EncryptUdpToggled)
                        } else {
                            cb
                        }
                    }
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        );

        let body = column![auth_card, ui::v_space(16.0), encrypt_card].spacing(0);
        ui::page_body("Security", body)
    }

    fn advanced_page(&self) -> Element<'_, Message> {
        let slider_row = row![
            slider(10..=1000, self.keepalive_interval_ms, Message::KeepaliveIntervalChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{} ms", self.keepalive_interval_ms)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let keepalive_field = column![
            text("Keepalive interval").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("A low setting may improve latency on some wireless networks, but sends more traffic."),
            ui::v_space(16.0),
            slider_row,
        ]
        .spacing(0);

        let key_repeat_row = row![
            slider(10..=1000, self.key_repeat_interval_ms, Message::KeyRepeatIntervalChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{} ms", self.key_repeat_interval_ms)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let key_repeat_field = column![
            text("Key repeat interval").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("Lower values make the server less likely to time out held keys on unreliable networks, but increase overhead."),
            ui::v_space(16.0),
            key_repeat_row,
        ]
        .spacing(0);

        let timeout_field = column![
            text("Reconnect timeout").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("How long to keep trying to reconnect after the server drops."),
            ui::v_space(16.0),
            text_input("30", &self.reconnect_timeout_secs)
                .on_input(Message::ReconnectTimeoutChanged)
                .padding(12)
                .size(14)
                .width(Length::Fixed(140.0)),
        ]
        .spacing(0);

        let batch_size_row = row![
            slider(1..=20, self.mouse_batch_size, Message::MouseBatchSizeChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{}", self.mouse_batch_size)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let batch_size_field = column![
            text("Mouse movement max batch size").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("Higher values decrease UDP overhead but may cause slight lag at lower pointer speeds."),
            ui::v_space(16.0),
            batch_size_row,
        ]
        .spacing(0);

        let batch_redundancy_row = row![
            slider(0..=50, self.batch_redundancy, Message::BatchRedundancyChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{}", self.batch_redundancy)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let batch_redundancy_field = column![
            text("Redundant batches").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("Include previously sent batches in each UDP packet to improve reliablity at the cost of latency."),
            ui::v_space(16.0),
            batch_redundancy_row,
        ]
        .spacing(0);

        let drop_row = row![
            slider(0..=100, self.udp_drop_percent, Message::UdpDropPercentChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{}%", self.udp_drop_percent)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let drop_field = column![
            text("UDP drop rate").size(16).color(mt::ON_SURFACE),
            ui::v_space(4.0),
            ui::helper_text("Percentage of UDP packets to drop for testing reliability."),
            ui::v_space(16.0),
            drop_row,
        ]
        .spacing(0);

        let network_card = ui::card(
            column![
                ui::card_title("Network"),
                ui::v_space(12.0),
                timeout_field,
            ]
            .spacing(0),
        );

        let perf_card = ui::card(
            column![
                ui::card_title("Performance"),
                ui::v_space(12.0),
                keepalive_field,
                ui::v_space(16.0),
                key_repeat_field,
                ui::v_space(16.0),
                batch_size_field,
                ui::v_space(16.0),
                batch_redundancy_field,
            ]
            .spacing(0),
        );

        let testing_card = ui::card(
            column![
                ui::card_title("Testing"),
                ui::v_space(12.0),
                drop_field,
            ]
            .spacing(0),
        );

        let body = column![
            network_card,
            ui::v_space(16.0),
            perf_card,
            ui::v_space(16.0),
            testing_card,
        ]
        .spacing(0);
        ui::page_body("Advanced", body)
    }
}

fn iced_to_wire(
    event: &iced::Event,
    last_cursor: &mut Option<Point>,
    pressed_keys: &mut HashSet<u16>,
    pressed_mouse_buttons: &mut HashSet<u8>,
    scale: (f32, f32),
    window_size: Option<iced::Size>,
    is_window_mode: bool,
    natural_scroll: bool,
) -> Option<crate::net::Event> {
    use iced::keyboard;
    use iced::mouse;

    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, physical_key, .. }) => {
            let code = physical_key_to_evdev(physical_key)
                .or_else(|| key_to_evdev(key))?;
            if pressed_keys.insert(code) {
                Some(crate::net::Event::KeyDown(code))
            } else {
                None
            }
        }
        iced::Event::Keyboard(keyboard::Event::KeyReleased { key, physical_key, .. }) => {
            let code = physical_key_to_evdev(physical_key)
                .or_else(|| key_to_evdev(key))?;
            pressed_keys.remove(&code);
            Some(crate::net::Event::KeyUp(code))
        }
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
            if is_window_mode {
                let (ww, wh) = window_size.map(|s| (s.width, s.height)).unwrap_or((1.0, 1.0));
                if ww > 0.0 && wh > 0.0 {
                    let x = ((position.x / ww) * 65535.0).clamp(0.0, 65535.0) as u16;
                    let y = ((position.y / wh) * 65535.0).clamp(0.0, 65535.0) as u16;
                    *last_cursor = Some(*position);
                    Some(crate::net::Event::MouseAbs { x, y })
                } else {
                    *last_cursor = Some(*position);
                    None
                }
            } else {
                let result = last_cursor.map(|prev| {
                    let dx = ((position.x - prev.x) * scale.0).round() as i16;
                    let dy = ((position.y - prev.y) * scale.1).round() as i16;
                    crate::net::Event::MouseMove { dx, dy }
                });
                if last_cursor.is_none() {
                    println!("[client] CursorMoved: last_cursor is None, no delta computed");
                }
                *last_cursor = Some(*position);
                result.filter(|e| !matches!(e, crate::net::Event::MouseMove { dx: 0, dy: 0 }))
            }
        }
        iced::Event::Mouse(mouse::Event::CursorLeft) => {
            *last_cursor = None;
            None
        }
        iced::Event::Mouse(mouse::Event::ButtonPressed(b)) => {
            let button = map_iced_button(b);
            if pressed_mouse_buttons.insert(button) {
                Some(crate::net::Event::MouseButton { button, pressed: true })
            } else {
                None
            }
        }
        iced::Event::Mouse(mouse::Event::ButtonReleased(b)) => {
            let button = map_iced_button(b);
            pressed_mouse_buttons.remove(&button);
            Some(crate::net::Event::MouseButton { button, pressed: false })
        }
        iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let (x, y) = match delta {
                mouse::ScrollDelta::Lines { x, y } => (x.round() as i32, y.round() as i32),
                mouse::ScrollDelta::Pixels { x, y } => {
                    ((x / 10.0).round() as i32, (y / 10.0).round() as i32)
                }
            };
            let dx = x.clamp(-127, 127) as i8;
            let mut dy = y.clamp(-127, 127) as i8;
            if natural_scroll {
                dy = -dy;
            }
            (dx != 0 || dy != 0).then_some(crate::net::Event::Wheel { dx, dy })
        }
        _ => None,
    }
}

fn input_event_to_wire(
    event: &crate::input::InputEvent,
    pressed_keys: &mut HashSet<u16>,
    pressed_mouse_buttons: &mut HashSet<u8>,
    sensitivity: f32,
    natural_scroll: bool,
) -> Option<crate::net::Event> {
    use crate::input::InputEvent;
    match event {
        InputEvent::KeyPress { keycode } => {
            // X11 keycodes are offset by 8 from Linux evdev scancodes.
            let code = keycode.saturating_sub(8) as u16;
            if pressed_keys.insert(code) {
                Some(crate::net::Event::KeyDown(code))
            } else {
                None
            }
        }
        InputEvent::KeyRelease { keycode } => {
            let code = keycode.saturating_sub(8) as u16;
            pressed_keys.remove(&code);
            Some(crate::net::Event::KeyUp(code))
        }
        InputEvent::MouseMove { dx, dy } => Some(crate::net::Event::MouseMove {
            dx: ((*dx as f32) * sensitivity).round() as i16,
            dy: ((*dy as f32) * sensitivity).round() as i16,
        }),
        InputEvent::Wheel { dx, dy } => {
            // X11/Wayland backends use dy > 0 = down, but iced uses dy > 0 = up.
            // Negate to match window mode convention, then apply natural scroll.
            let mut dy = -*dy;
            if natural_scroll {
                dy = -dy;
            }
            Some(crate::net::Event::Wheel { dx: *dx, dy })
        }
        InputEvent::MouseButton { button, pressed: true } => {
            if pressed_mouse_buttons.insert(*button) {
                Some(crate::net::Event::MouseButton { button: *button, pressed: true })
            } else {
                None
            }
        }
        InputEvent::MouseButton { button, pressed: false } => {
            pressed_mouse_buttons.remove(button);
            Some(crate::net::Event::MouseButton { button: *button, pressed: false })
        }
        InputEvent::HotkeyToggled { .. } | InputEvent::BackendError(_) => None,
    }
}

/// Map a logical `Key` to an evdev scancode (fallback when physical key is unknown).
fn key_to_evdev(key: &Key) -> Option<u16> {
    match key {
        Key::Character(s) => char_to_evdev(s.chars().next()?),
        Key::Named(n) => named_key_to_evdev(n),
        Key::Unidentified => None,
    }
}

fn char_to_evdev(c: char) -> Option<u16> {
    Some(match c {
        'a' | 'A' => 30,
        'b' | 'B' => 48,
        'c' | 'C' => 46,
        'd' | 'D' => 32,
        'e' | 'E' => 18,
        'f' | 'F' => 33,
        'g' | 'G' => 34,
        'h' | 'H' => 35,
        'i' | 'I' => 23,
        'j' | 'J' => 36,
        'k' | 'K' => 37,
        'l' | 'L' => 38,
        'm' | 'M' => 50,
        'n' | 'N' => 49,
        'o' | 'O' => 24,
        'p' | 'P' => 25,
        'q' | 'Q' => 16,
        'r' | 'R' => 19,
        's' | 'S' => 31,
        't' | 'T' => 20,
        'u' | 'U' => 22,
        'v' | 'V' => 47,
        'w' | 'W' => 17,
        'x' | 'X' => 45,
        'y' | 'Y' => 21,
        'z' | 'Z' => 44,
        '1' | '!' => 2,
        '2' | '@' => 3,
        '3' | '#' => 4,
        '4' | '$' => 5,
        '5' | '%' => 6,
        '6' | '^' => 7,
        '7' | '&' => 8,
        '8' | '*' => 9,
        '9' | '(' => 10,
        '0' | ')' => 11,
        '-' | '_' => 12,
        '=' | '+' => 13,
        '[' | '{' => 26,
        ']' | '}' => 27,
        '\\' | '|' => 43,
        ';' | ':' => 39,
        '\'' | '"' => 40,
        '`' | '~' => 41,
        ',' | '<' => 51,
        '.' | '>' => 52,
        '/' | '?' => 53,
        ' ' => 57,
        _ => return None,
    })
}

fn named_key_to_evdev(n: &iced::keyboard::key::Named) -> Option<u16> {
    use iced::keyboard::key::Named;
    Some(match n {
        Named::Enter => 28,
        Named::Tab => 15,
        Named::Space => 57,
        Named::ArrowDown => 108,
        Named::ArrowLeft => 105,
        Named::ArrowRight => 106,
        Named::ArrowUp => 103,
        Named::End => 107,
        Named::Home => 102,
        Named::PageDown => 109,
        Named::PageUp => 104,
        Named::Backspace => 14,
        Named::Delete => 111,
        Named::Insert => 110,
        Named::Escape => 1,
        Named::Pause => 119,
        Named::PrintScreen => 99,
        Named::ContextMenu => 127,
        Named::Help => 138,
        Named::CapsLock => 58,
        Named::NumLock => 69,
        Named::ScrollLock => 70,
        Named::Alt => 56,          // left alt fallback
        Named::AltGraph => 100,    // right alt
        Named::Control => 29,      // left ctrl fallback
        Named::Shift => 42,        // left shift fallback
        Named::Super => 125,       // left super fallback
        Named::F1 => 59,
        Named::F2 => 60,
        Named::F3 => 61,
        Named::F4 => 62,
        Named::F5 => 63,
        Named::F6 => 64,
        Named::F7 => 65,
        Named::F8 => 66,
        Named::F9 => 67,
        Named::F10 => 68,
        Named::F11 => 87,
        Named::F12 => 88,
        Named::F13 => 183,
        Named::F14 => 184,
        Named::F15 => 185,
        Named::F16 => 186,
        Named::F17 => 187,
        Named::F18 => 188,
        Named::F19 => 189,
        Named::F20 => 190,
        Named::F21 => 191,
        Named::F22 => 192,
        Named::F23 => 193,
        Named::F24 => 194,
        Named::Convert => 92,
        Named::NonConvert => 94,
        Named::KanaMode => 93,
        Named::HangulMode => 122,
        Named::HanjaMode => 123,
        Named::JunjaMode => 129,
        Named::Hiragana => 91,
        Named::Katakana => 90,
        Named::HiraganaKatakana => 93,
        Named::KanjiMode => 93,
        _ => return None,
    })
}

/// Convert a physical key to an evdev scancode.
///
/// On Linux this uses the raw scancode so the mapping is layout-independent
/// and distinguishes left/right modifiers.
fn physical_key_to_evdev(physical: &iced::keyboard::key::Physical) -> Option<u16> {
    use iced::keyboard::key::{NativeCode, Physical};
    match physical {
        Physical::Code(code) => code_to_evdev(code),
        Physical::Unidentified(NativeCode::Xkb(k)) => Some(*k as u16),
        _ => None,
    }
}

/// Map an iced `Code` to a Linux evdev scancode.
///
/// This table is the inverse of winit's `scancode_to_physicalkey` mapping.
fn code_to_evdev(code: &iced::keyboard::key::Code) -> Option<u16> {
    use iced::keyboard::key::Code;
    let scancode = match code {
        Code::Backquote => 41,
        Code::Backslash => 43,
        Code::BracketLeft => 26,
        Code::BracketRight => 27,
        Code::Comma => 51,
        Code::Digit0 => 11,
        Code::Digit1 => 2,
        Code::Digit2 => 3,
        Code::Digit3 => 4,
        Code::Digit4 => 5,
        Code::Digit5 => 6,
        Code::Digit6 => 7,
        Code::Digit7 => 8,
        Code::Digit8 => 9,
        Code::Digit9 => 10,
        Code::Equal => 13,
        Code::IntlBackslash => 86,
        Code::IntlRo => 89,
        Code::IntlYen => 124,
        Code::KeyA => 30,
        Code::KeyB => 48,
        Code::KeyC => 46,
        Code::KeyD => 32,
        Code::KeyE => 18,
        Code::KeyF => 33,
        Code::KeyG => 34,
        Code::KeyH => 35,
        Code::KeyI => 23,
        Code::KeyJ => 36,
        Code::KeyK => 37,
        Code::KeyL => 38,
        Code::KeyM => 50,
        Code::KeyN => 49,
        Code::KeyO => 24,
        Code::KeyP => 25,
        Code::KeyQ => 16,
        Code::KeyR => 19,
        Code::KeyS => 31,
        Code::KeyT => 20,
        Code::KeyU => 22,
        Code::KeyV => 47,
        Code::KeyW => 17,
        Code::KeyX => 45,
        Code::KeyY => 21,
        Code::KeyZ => 44,
        Code::Minus => 12,
        Code::Period => 52,
        Code::Quote => 40,
        Code::Semicolon => 39,
        Code::Slash => 53,
        Code::AltLeft => 56,
        Code::AltRight => 100,
        Code::Backspace => 14,
        Code::CapsLock => 58,
        Code::ContextMenu => 127,
        Code::ControlLeft => 29,
        Code::ControlRight => 97,
        Code::Enter => 28,
        Code::SuperLeft => 125,
        Code::SuperRight => 126,
        Code::ShiftLeft => 42,
        Code::ShiftRight => 54,
        Code::Space => 57,
        Code::Tab => 15,
        Code::Convert => 92,
        Code::KanaMode => 93,
        Code::Lang1 => 122,
        Code::Lang2 => 123,
        Code::Lang3 => 90,
        Code::Lang4 => 91,
        Code::Lang5 => 85,
        Code::NonConvert => 94,
        Code::Delete => 111,
        Code::End => 107,
        Code::Help => 138,
        Code::Home => 102,
        Code::Insert => 110,
        Code::PageDown => 109,
        Code::PageUp => 104,
        Code::ArrowDown => 108,
        Code::ArrowLeft => 105,
        Code::ArrowRight => 106,
        Code::ArrowUp => 103,
        Code::NumLock => 69,
        Code::Numpad0 => 82,
        Code::Numpad1 => 79,
        Code::Numpad2 => 80,
        Code::Numpad3 => 81,
        Code::Numpad4 => 75,
        Code::Numpad5 => 76,
        Code::Numpad6 => 77,
        Code::Numpad7 => 71,
        Code::Numpad8 => 72,
        Code::Numpad9 => 73,
        Code::NumpadAdd => 78,
        Code::NumpadComma => 121,
        Code::NumpadDecimal => 83,
        Code::NumpadDivide => 98,
        Code::NumpadEnter => 96,
        Code::NumpadEqual => 117,
        Code::NumpadMultiply => 55,
        Code::NumpadSubtract => 74,
        Code::Pause => 119,
        Code::PrintScreen => 99,
        Code::ScrollLock => 70,
        Code::F1 => 59,
        Code::F2 => 60,
        Code::F3 => 61,
        Code::F4 => 62,
        Code::F5 => 63,
        Code::F6 => 64,
        Code::F7 => 65,
        Code::F8 => 66,
        Code::F9 => 67,
        Code::F10 => 68,
        Code::F11 => 87,
        Code::F12 => 88,
        Code::F13 => 183,
        Code::F14 => 184,
        Code::F15 => 185,
        Code::F16 => 186,
        Code::F17 => 187,
        Code::F18 => 188,
        Code::F19 => 189,
        Code::F20 => 190,
        Code::F21 => 191,
        Code::F22 => 192,
        Code::F23 => 193,
        Code::F24 => 194,
        Code::AudioVolumeDown => 114,
        Code::AudioVolumeMute => 113,
        Code::AudioVolumeUp => 115,
        Code::MediaPlayPause => 164,
        Code::MediaStop => 166,
        Code::MediaTrackNext => 163,
        Code::MediaTrackPrevious => 165,
        _ => return None,
    };
    Some(scancode)
}

fn map_iced_button(b: &iced::mouse::Button) -> u8 {
    use iced::mouse::Button;
    match b {
        Button::Left => 1,
        Button::Right => 3,
        Button::Middle => 2,
        Button::Back => 8,
        Button::Forward => 9,
        Button::Other(n) => (*n).min(255) as u8,
    }
}

fn format_chord(key: &Key, modifiers: Modifiers) -> Option<String> {
    use iced::keyboard::key::Named;

    let key_str = match key {
        Key::Named(named) => match named {
            Named::Control
            | Named::Shift
            | Named::Alt
            | Named::Super
            | Named::Hyper
            | Named::Meta
            | Named::Escape => return None,
            Named::Space => "Space",
            Named::Enter => "Enter",
            Named::Tab => "Tab",
            Named::Backspace => "Backspace",
            Named::Delete => "Delete",
            Named::Insert => "Insert",
            Named::Home => "Home",
            Named::End => "End",
            Named::PageUp => "Page Up",
            Named::PageDown => "Page Down",
            Named::ArrowLeft => "Left",
            Named::ArrowRight => "Right",
            Named::ArrowUp => "Up",
            Named::ArrowDown => "Down",
            Named::F1 => "F1",
            Named::F2 => "F2",
            Named::F3 => "F3",
            Named::F4 => "F4",
            Named::F5 => "F5",
            Named::F6 => "F6",
            Named::F7 => "F7",
            Named::F8 => "F8",
            Named::F9 => "F9",
            Named::F10 => "F10",
            Named::F11 => "F11",
            Named::F12 => "F12",
            Named::PrintScreen => "Print Screen",
            Named::ScrollLock => "Scroll Lock",
            Named::Pause => "Pause",
            Named::CapsLock => "Caps Lock",
            Named::NumLock => "Num Lock",
            _ => return None,
        }
        .to_string(),
        Key::Character(c) => c.to_uppercase().to_string(),
        Key::Unidentified => return None,
    };

    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Super");
    }
    parts.push(&key_str);
    Some(parts.join("+"))
}
