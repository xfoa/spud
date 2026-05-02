use iced::keyboard::{Key, Modifiers};
use iced::widget::{checkbox, column, container, row, slider, text, text_input};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::components as ui;
use crate::config::{hash_passphrase, CaptureMode, ClientConfig};
use crate::icons;
use crate::theme as mt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Connection,
    Input,
    Hotkeys,
    Security,
}

impl Page {
    const ALL: [Page; 4] = [Page::Connection, Page::Input, Page::Hotkeys, Page::Security];

    fn label(self) -> &'static str {
        match self {
            Page::Connection => "Connection",
            Page::Input => "Input",
            Page::Hotkeys => "Hotkeys",
            Page::Security => "Security",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Connection => icons::PLUG,
            Page::Input => icons::COMPUTER_MOUSE,
            Page::Hotkeys => icons::KEYBOARD,
            Page::Security => icons::SHIELD_HALVED,
        }
    }
}

const CAPTURE_MODES: [CaptureMode; 2] = [
    CaptureMode::Hotkey,
    CaptureMode::Focus,
];

#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    pub name: String,
    pub host: String,
    pub port: String,
    pub address: String,
    pub icon: char,
}

impl DiscoveredServer {
    fn new(name: &str, host: &str, port: &str, icon: char) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            port: port.to_string(),
            address: format!("{}:{}", host, port),
            icon,
        }
    }
}

fn example_servers() -> Vec<DiscoveredServer> {
    vec![
        DiscoveredServer::new("studio-mac", "studio-mac.local", "7878", icons::DESKTOP),
        DiscoveredServer::new("thinkpad-x1", "thinkpad-x1.local", "7878", icons::LAPTOP),
        DiscoveredServer::new("office-tower", "192.168.1.42", "7878", icons::DESKTOP),
        DiscoveredServer::new("homelab", "homelab.local", "7878", icons::SERVER),
    ]
}

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
    OpenHotkeyDialog,
    CloseHotkeyDialog,
    ConfirmHotkey,
    HotkeyInput(Key, Modifiers),
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
            discovered: example_servers(),
            hotkey_dialog_open: false,
            pending_hotkey: String::new(),
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
            Message::Connect => self.connected = true,
            Message::Disconnect => self.connected = false,
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
        }
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

    pub fn view_content(&self, content_width: f32) -> Element<'_, Message> {
        match self.page {
            Page::Connection => self.connection_page(content_width),
            Page::Input => self.input_page(),
            Page::Hotkeys => self.hotkeys_page(),
            Page::Security => self.security_page(),
        }
    }

    fn connection_page(&self, content_width: f32) -> Element<'_, Message> {
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
            ui::filled_button("Connect", can_connect.then_some(Message::Connect))
        };

        let connection_card = ui::card(
            column![
                status_row,
                ui::v_space(16.0),
                host_field,
                ui::v_space(12.0),
                port_field,
                ui::v_space(20.0),
                row![ui::h_space_fill(), action].width(Length::Fill),
            ]
            .spacing(0),
        );

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
