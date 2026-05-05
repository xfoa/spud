use std::collections::HashSet;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{checkbox, column, container, row, slider, text, text_input};
use iced::{Background, Border, Color, Element, Length, Padding, Point, Shadow, Vector};

use crate::components as ui;
use crate::config::{hash_passphrase, CaptureMode, ClientConfig};
use crate::discovery::{self, DiscoveredServer};
use crate::icons;
use crate::theme as mt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connection,
    Capture,
    Mouse,
    Security,
}

impl Page {
    const ALL: [Page; 4] = [Page::Connection, Page::Capture, Page::Mouse, Page::Security];

    fn label(self) -> &'static str {
        match self {
            Page::Connection => "Connection",
            Page::Mouse => "Mouse",
            Page::Capture => "Capture",
            Page::Security => "Security",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Connection => icons::PLUG,
            Page::Mouse => icons::COMPUTER_MOUSE,
            Page::Capture => icons::KEYBOARD,
            Page::Security => icons::SHIELD_HALVED,
        }
    }
}

const CAPTURE_MODES: [CaptureMode; 2] = [
    CaptureMode::Hotkey,
    CaptureMode::Focus,
];


#[derive(Debug, Clone)]
pub enum Message {
    SelectPage(Page),
    HostChanged(String),
    PortChanged(String),
    Connect,
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
    HeartbeatTick,
}

pub struct State {
    page: Page,
    host: String,
    port: String,
    connected: bool,
    sensitivity: f32,
    natural_scroll: bool,
    capture_mode: CaptureMode,
    hotkey: String,
    require_auth: bool,
    passphrase: String,
    passphrase_hash: String,
    discovered: Vec<DiscoveredServer>,
    pub hotkey_dialog_open: bool,
    pending_hotkey: String,
    sender: Option<crate::net::Sender>,
    last_cursor: Option<Point>,
    last_error: Option<String>,
    pressed_keys: HashSet<String>,
    cursor_inside: bool,
    heartbeat_interval_ms: u64,
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
            sensitivity: cfg.sensitivity.parse().unwrap_or(1.0),
            natural_scroll: cfg.natural_scroll,
            capture_mode: cfg.capture_mode,
            hotkey: cfg.hotkey.clone(),
            require_auth: cfg.require_auth,
            passphrase: String::new(),
            passphrase_hash: cfg.passphrase_hash.clone(),
            discovered: Vec::new(),
            hotkey_dialog_open: false,
            pending_hotkey: String::new(),
            sender: None,
            last_cursor: None,
            last_error: None,
            pressed_keys: HashSet::new(),
            cursor_inside: true,
            heartbeat_interval_ms: 500,
        }
    }

    pub fn to_config(&self) -> ClientConfig {
        let passphrase_hash = if self.passphrase.is_empty() {
            self.passphrase_hash.clone()
        } else {
            hash_passphrase(&self.passphrase)
        };
        ClientConfig {
            host: self.host.clone(),
            port: self.port.clone(),
            sensitivity: format!("{:.2}", self.sensitivity),
            natural_scroll: self.natural_scroll,
            capture_mode: self.capture_mode,
            hotkey: self.hotkey.clone(),
            require_auth: self.require_auth,
            passphrase_hash,
        }
    }
}

impl State {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectPage(p) => self.page = p,
            Message::HostChanged(s) => self.host = s,
            Message::PortChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.port = s;
                }
            }
            Message::Connect => {
                let port = self.port.parse::<u16>().unwrap_or(7878);
                self.last_error = None;
                match crate::net::Sender::connect(&self.host, port) {
                    Ok(s) => {
                        self.heartbeat_interval_ms = u64::from(s.key_timeout_ms) / 2;
                        self.heartbeat_interval_ms = self.heartbeat_interval_ms.max(50);
                        self.sender = Some(s);
                        self.connected = true;
                    }
                    Err(e) => {
                        self.last_error = Some(format!("{e}"));
                    }
                }
            }
            Message::Disconnect => {
                self.connected = false;
                self.sender = None;
                self.last_cursor = None;
                self.last_error = None;
                self.pressed_keys.clear();
                self.heartbeat_interval_ms = 500;
            }
            Message::ConnectionLost => {
                if self.connected {
                    self.connected = false;
                    self.sender = None;
                    self.last_cursor = None;
                    self.pressed_keys.clear();
                    self.heartbeat_interval_ms = 500;
                    self.last_error = Some("Server closed the connection.".to_string());
                }
            }
            Message::SensitivityChanged(v) => self.sensitivity = v,
            Message::NaturalScrollToggled(v) => self.natural_scroll = v,
            Message::CaptureModeChanged(m) => self.capture_mode = m,
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.passphrase = s,
            Message::SelectDiscovered(i) => {
                if let Some(server) = self.discovered.get(i) {
                    self.host = server.host.clone();
                    self.port = server.port.clone();
                }
            }
            Message::DiscoveryEvent(event) => match event {
                discovery::Event::Found(server) => {
                    self.discovered.retain(|s| s.fullname != server.fullname);
                    self.discovered.push(server);
                    self.discovered.sort_by(|a, b| a.name.cmp(&b.name));
                }
                discovery::Event::Lost(fullname) => {
                    self.discovered.retain(|s| s.fullname != fullname);
                }
            },
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
                if let iced::Event::Mouse(iced::mouse::Event::CursorEntered) = &event {
                    self.cursor_inside = true;
                    return;
                }
                if let iced::Event::Mouse(iced::mouse::Event::CursorLeft) = &event {
                    self.cursor_inside = false;
                    if let Some(sender) = &self.sender {
                        for name in &self.pressed_keys {
                            sender.send(&crate::net::Event::KeyUp(name.clone()));
                        }
                    }
                    self.pressed_keys.clear();
                    return;
                }
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) = &event
                {
                    if self.capture_mode == CaptureMode::Hotkey
                        && format_chord(key, *modifiers).as_deref() == Some(self.hotkey.as_str())
                    {
                        crate::input::toggle_wayland_grab();
                        return;
                    }
                }
                let forward = match self.capture_mode {
                    CaptureMode::Focus => self.cursor_inside,
                    CaptureMode::Hotkey => crate::input::is_wayland_grabbed(),
                };
                if forward {
                    if let Some(wire) =
                        iced_to_wire(&event, &mut self.last_cursor, &mut self.pressed_keys)
                    {
                        if let Some(sender) = &self.sender {
                            sender.send(&wire);
                        }
                    }
                }
            }
            Message::HotkeyEvent(event) => {
                match event {
                    crate::input::InputEvent::HotkeyToggled { grabbed: false } => {
                        if let Some(sender) = &self.sender {
                            for name in &self.pressed_keys {
                                sender.send(&crate::net::Event::KeyUp(name.clone()));
                            }
                        }
                        self.pressed_keys.clear();
                    }
                    _ => {
                        if let Some(wire) = input_event_to_wire(&event, &mut self.pressed_keys) {
                            if let Some(sender) = &self.sender {
                                sender.send(&wire);
                            }
                        }
                    }
                }
            }
            Message::HeartbeatTick => {
                if let Some(sender) = &self.sender {
                    for name in &self.pressed_keys {
                        sender.send(&crate::net::Event::KeyRepeat(name.clone()));
                    }
                }
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn is_capturing_focused(&self) -> bool {
        self.connected && self.capture_mode == CaptureMode::Focus
    }

    pub fn is_capturing_hotkey(&self) -> bool {
        self.connected && self.capture_mode == CaptureMode::Hotkey
    }

    pub fn heartbeat_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.heartbeat_interval_ms)
    }

    pub fn hotkey_string(&self) -> &str {
        &self.hotkey
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
        }
    }

    fn connection_page(&self, content_width: f32, server_running: bool) -> Element<'_, Message> {
        let status_label = if self.connected && !self.require_auth {
            "Connected (insecure)"
        } else if self.connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let status_color = if self.connected { mt::SUCCESS } else { mt::ON_SURFACE_VARIANT };

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
        if !self.connected {
            host_input = host_input.on_input(Message::HostChanged);
        }
        let host_field: Element<Message> = if !self.connected && self.host.is_empty() {
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
        if !self.connected {
            port_input = port_input.on_input(Message::PortChanged);
        }
        let port_out_of_range = !self.port.is_empty()
            && !self.port.parse::<u16>().is_ok_and(|p| p > 0);

        let port_field: Element<Message> = if !self.connected && self.port.is_empty() {
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

        let action: Element<Message> = if self.connected {
            ui::outlined_button("Disconnect", Message::Disconnect)
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

        if server_running && !self.connected {
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

        let connection_card = ui::card(column(conn_items).spacing(0));

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
                    let selected = self.host == s.host && self.port == s.port;
                    let on_press = (!self.connected).then_some(Message::SelectDiscovered(idx));
                    ui::server_tile(
                        s.icon,
                        s.name.as_str(),
                        s.address.as_str(),
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
                text("Discovered servers").size(16).color(mt::ON_SURFACE),
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
                text("Mouse sensitivity").size(16).color(mt::ON_SURFACE),
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

        let body = column![sens_card, ui::v_space(16.0), scroll_card].spacing(0);
        ui::page_body("Input", body)
    }

    fn hotkeys_page(&self) -> Element<'_, Message> {
        let capture_card = ui::card(
            column![
                text("Capture mode").size(16).color(mt::ON_SURFACE),
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

        let hotkey_card = ui::card(
            column![
                text("Capture hotkey").size(16).color(mt::ON_SURFACE),
                ui::v_space(4.0),
                ui::helper_text("Used when capture mode is set to Hotkey."),
                ui::v_space(16.0),
                row![
                    text(&self.hotkey).size(14).color(mt::ON_SURFACE),
                    ui::h_space_fill(),
                    ui::outlined_button("Record hotkey", Message::OpenHotkeyDialog),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0),
        );

        let body = column![capture_card, ui::v_space(16.0), hotkey_card].spacing(0);
        ui::page_body("Hotkeys", body)
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
                    ui::outlined_button("Cancel", Message::CloseHotkeyDialog),
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

        let backdrop = container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(mt::with_alpha(Color::BLACK, 0.45))),
                ..Default::default()
            });

        Some(backdrop.into())
    }

    fn security_page(&self) -> Element<'_, Message> {
        let auth_card = ui::card(
            row![
                column![
                    text("Require authentication").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text("Send a passphrase when connecting to the server."),
                ]
                .width(Length::Fill),
                checkbox(self.require_auth).on_toggle(Message::RequireAuthToggled),
            ]
            .align_y(iced::Alignment::Center),
        );

        let mut passphrase_items: Vec<Element<Message>> = vec![
            text("Passphrase").size(16).color(mt::ON_SURFACE).into(),
            ui::v_space(4.0).into(),
            ui::helper_text("Must match the passphrase set on the server.").into(),
            ui::v_space(16.0).into(),
            text_input("Enter passphrase", &self.passphrase)
                .on_input(Message::PassphraseChanged)
                .secure(true)
                .padding(12)
                .size(14)
                .into(),
        ];

        if self.passphrase.is_empty() {
            passphrase_items.push(ui::v_space(8.0).into());
            if self.passphrase_hash.is_empty() {
                if self.require_auth {
                    passphrase_items.push(
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
            } else {
                passphrase_items.push(
                    row![
                        text(icons::LOCK)
                            .font(icons::FA_SOLID)
                            .size(11)
                            .color(mt::SUCCESS),
                        text("Passphrase is set. Type to change.")
                            .size(12)
                            .color(mt::SUCCESS),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center)
                    .into(),
                );
            }
        }

        let passphrase_card = ui::card(column(passphrase_items).spacing(0));

        let body = column![auth_card, ui::v_space(16.0), passphrase_card].spacing(0);
        ui::page_body("Security", body)
    }
}

fn iced_to_wire(
    event: &iced::Event,
    last_cursor: &mut Option<Point>,
    pressed_keys: &mut HashSet<String>,
) -> Option<crate::net::Event> {
    use iced::keyboard;
    use iced::mouse;

    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
            let name = key_to_string(key);
            if pressed_keys.insert(name.clone()) {
                Some(crate::net::Event::KeyDown(name))
            } else {
                None
            }
        }
        iced::Event::Keyboard(keyboard::Event::KeyReleased { key, .. }) => {
            let name = key_to_string(key);
            pressed_keys.remove(&name);
            Some(crate::net::Event::KeyUp(name))
        }
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
            let result = last_cursor.map(|prev| crate::net::Event::MouseMove {
                dx: (position.x - prev.x).round() as i16,
                dy: (position.y - prev.y).round() as i16,
            });
            *last_cursor = Some(*position);
            result.filter(|e| !matches!(e, crate::net::Event::MouseMove { dx: 0, dy: 0 }))
        }
        iced::Event::Mouse(mouse::Event::CursorLeft) => {
            *last_cursor = None;
            None
        }
        iced::Event::Mouse(mouse::Event::ButtonPressed(b)) => Some(crate::net::Event::MouseButton {
            button: map_iced_button(b),
            pressed: true,
        }),
        iced::Event::Mouse(mouse::Event::ButtonReleased(b)) => {
            Some(crate::net::Event::MouseButton {
                button: map_iced_button(b),
                pressed: false,
            })
        }
        iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let (x, y) = match delta {
                mouse::ScrollDelta::Lines { x, y } => (x.round() as i32, y.round() as i32),
                mouse::ScrollDelta::Pixels { x, y } => {
                    ((x / 10.0).round() as i32, (y / 10.0).round() as i32)
                }
            };
            let dx = x.clamp(-127, 127) as i8;
            let dy = y.clamp(-127, 127) as i8;
            (dx != 0 || dy != 0).then_some(crate::net::Event::Wheel { dx, dy })
        }
        _ => None,
    }
}

fn input_event_to_wire(
    event: &crate::input::InputEvent,
    pressed_keys: &mut HashSet<String>,
) -> Option<crate::net::Event> {
    use crate::input::InputEvent;
    match event {
        InputEvent::KeyPress { keycode } => {
            let name = format!("evdev:{keycode}");
            if pressed_keys.insert(name.clone()) {
                Some(crate::net::Event::KeyDown(name))
            } else {
                None
            }
        }
        InputEvent::KeyRelease { keycode } => {
            let name = format!("evdev:{keycode}");
            pressed_keys.remove(&name);
            Some(crate::net::Event::KeyUp(name))
        }
        InputEvent::MouseMove { dx, dy } => Some(crate::net::Event::MouseMove {
            dx: *dx,
            dy: *dy,
        }),
        InputEvent::MouseButton { button, pressed } => Some(crate::net::Event::MouseButton {
            button: *button,
            pressed: *pressed,
        }),
        InputEvent::HotkeyToggled { .. } | InputEvent::BackendError(_) => None,
    }
}

fn key_to_string(key: &Key) -> String {
    match key {
        Key::Character(s) => s.to_string(),
        Key::Named(n) => format!("{n:?}"),
        Key::Unidentified => "Unidentified".to_string(),
    }
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
