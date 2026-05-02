use iced::widget::{checkbox, column, pick_list, row, slider, text, text_input};
use iced::{Element, Length};

use crate::components as ui;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Hotkey,
    Always,
    EdgeOfScreen,
}

impl CaptureMode {
    const ALL: [CaptureMode; 3] = [
        CaptureMode::Hotkey,
        CaptureMode::Always,
        CaptureMode::EdgeOfScreen,
    ];
}

impl std::fmt::Display for CaptureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CaptureMode::Hotkey => "When hotkey is held",
            CaptureMode::Always => "Always capture",
            CaptureMode::EdgeOfScreen => "When pointer hits edge",
        };
        f.write_str(s)
    }
}

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
    HotkeyChanged(String),
    RequireAuthToggled(bool),
    PassphraseChanged(String),
    SelectDiscovered(usize),
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
    discovered: Vec<DiscoveredServer>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            page: Page::Connection,
            host: String::new(),
            port: "7878".to_string(),
            connected: false,
            sensitivity: 1.0,
            natural_scroll: false,
            capture_mode: CaptureMode::Hotkey,
            hotkey: "Ctrl+Alt+Space".to_string(),
            require_auth: true,
            passphrase: String::new(),
            discovered: example_servers(),
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
            Message::HotkeyChanged(s) => self.hotkey = s,
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.passphrase = s,
            Message::SelectDiscovered(i) => {
                if let Some(server) = self.discovered.get(i) {
                    self.host = server.host.clone();
                    self.port = server.port.clone();
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
                pick_list(
                    CaptureMode::ALL,
                    Some(self.capture_mode),
                    Message::CaptureModeChanged,
                )
                .padding(10)
                .width(Length::Fill),
            ]
            .spacing(0),
        );

        let hotkey_card = ui::card(
            column![
                text("Capture hotkey").size(16).color(mt::ON_SURFACE),
                ui::v_space(4.0),
                ui::helper_text("Used when capture mode is set to Hotkey."),
                ui::v_space(16.0),
                text_input("Press a chord", &self.hotkey)
                    .on_input(Message::HotkeyChanged)
                    .padding(12)
                    .size(14),
            ]
            .spacing(0),
        );

        let body = column![capture_card, ui::v_space(16.0), hotkey_card].spacing(0);
        ui::page_body("Hotkeys", body)
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

        let passphrase_card = ui::card(
            column![
                text("Passphrase").size(16).color(mt::ON_SURFACE),
                ui::v_space(4.0),
                ui::helper_text("Must match the passphrase set on the server."),
                ui::v_space(16.0),
                text_input("Enter passphrase", &self.passphrase)
                    .on_input(Message::PassphraseChanged)
                    .secure(true)
                    .padding(12)
                    .size(14),
            ]
            .spacing(0),
        );

        let body = column![auth_card, ui::v_space(16.0), passphrase_card].spacing(0);
        ui::page_body("Security", body)
    }
}
