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
}

impl Page {
    const ALL: [Page; 3] = [Page::Connection, Page::Input, Page::Hotkeys];

    fn label(self) -> &'static str {
        match self {
            Page::Connection => "Connection",
            Page::Input => "Input",
            Page::Hotkeys => "Hotkeys",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Connection => icons::PLUG,
            Page::Input => icons::COMPUTER_MOUSE,
            Page::Hotkeys => icons::KEYBOARD,
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

    pub fn view_content(&self) -> Element<'_, Message> {
        match self.page {
            Page::Connection => self.connection_page(),
            Page::Input => self.input_page(),
            Page::Hotkeys => self.hotkeys_page(),
        }
    }

    fn connection_page(&self) -> Element<'_, Message> {
        let status_label = if self.connected { "Connected" } else { "Disconnected" };
        let status_color = if self.connected { mt::SUCCESS } else { mt::ON_SURFACE_VARIANT };

        let status_row = row![
            text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
            text(status_label).size(14).color(status_color),
        ]
        .spacing(8);

        let host_field = column![
            ui::field_label("Server address"),
            text_input("e.g. 192.168.1.42 or hostname.local", &self.host)
                .on_input(Message::HostChanged)
                .padding(12)
                .size(14),
        ]
        .spacing(6);

        let port_field = column![
            ui::field_label("Port"),
            text_input("7878", &self.port)
                .on_input(Message::PortChanged)
                .padding(12)
                .size(14)
                .width(Length::Fixed(120.0)),
        ]
        .spacing(6);

        let action: Element<Message> = if self.connected {
            ui::outlined_button("Disconnect", Message::Disconnect)
        } else {
            ui::filled_button("Connect", Message::Connect)
        };

        let card_content = column![
            status_row,
            ui::v_space(16.0),
            host_field,
            ui::v_space(12.0),
            port_field,
            ui::v_space(20.0),
            row![ui::h_space_fill(), action].width(Length::Fill),
        ]
        .spacing(0);

        ui::page_body("Connection", ui::card(card_content))
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
}
