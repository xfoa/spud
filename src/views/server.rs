use iced::widget::{checkbox, column, row, text, text_input};
use iced::{Element, Length};

use crate::components as ui;
use crate::theme as mt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Status,
    Network,
    Security,
}

impl Page {
    const ALL: [Page; 3] = [Page::Status, Page::Network, Page::Security];

    fn label(self) -> &'static str {
        match self {
            Page::Status => "Status",
            Page::Network => "Network",
            Page::Security => "Security",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Status => "signal",
            Page::Network => "network-wired",
            Page::Security => "shield-halved",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectPage(Page),
    StartServer,
    StopServer,
    BindAddressChanged(String),
    PortChanged(String),
    DiscoverableToggled(bool),
    RequireAuthToggled(bool),
    PassphraseChanged(String),
}

pub struct State {
    page: Page,
    running: bool,
    bind_address: String,
    port: String,
    discoverable: bool,
    require_auth: bool,
    passphrase: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            page: Page::Status,
            running: false,
            bind_address: "0.0.0.0".to_string(),
            port: "7878".to_string(),
            discoverable: true,
            require_auth: true,
            passphrase: String::new(),
        }
    }
}

impl State {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectPage(p) => self.page = p,
            Message::StartServer => self.running = true,
            Message::StopServer => self.running = false,
            Message::BindAddressChanged(s) => self.bind_address = s,
            Message::PortChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.port = s;
                }
            }
            Message::DiscoverableToggled(v) => self.discoverable = v,
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.passphrase = s,
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
            Page::Status => self.status_page(),
            Page::Network => self.network_page(),
            Page::Security => self.security_page(),
        }
    }

    fn status_page(&self) -> Element<'_, Message> {
        let status_label = if self.running { "Listening" } else { "Stopped" };
        let status_color = if self.running { mt::SUCCESS } else { mt::ON_SURFACE_VARIANT };

        let action: Element<Message> = if self.running {
            ui::outlined_button("Stop server", Message::StopServer)
        } else {
            ui::filled_button("Start server", Message::StartServer)
        };

        let endpoint = format!("{}:{}", self.bind_address, self.port);

        let card_content = column![
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(status_label).size(14).color(status_color),
            ]
            .spacing(8),
            ui::v_space(8.0),
            row![
                text("Listening on:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(endpoint).size(14).color(mt::ON_SURFACE),
            ]
            .spacing(8),
            ui::v_space(20.0),
            row![ui::h_space_fill(), action].width(Length::Fill),
        ]
        .spacing(0);

        ui::page_body("Status", ui::card(card_content))
    }

    fn network_page(&self) -> Element<'_, Message> {
        let bind_field = column![
            ui::field_label("Bind address"),
            text_input("0.0.0.0", &self.bind_address)
                .on_input(Message::BindAddressChanged)
                .padding(12)
                .size(14),
            ui::v_space(4.0),
            ui::helper_text("Use 0.0.0.0 to listen on every interface."),
        ]
        .spacing(6);

        let port_field = column![
            ui::field_label("Port"),
            text_input("7878", &self.port)
                .on_input(Message::PortChanged)
                .padding(12)
                .size(14)
                .width(Length::Fixed(140.0)),
        ]
        .spacing(6);

        let bind_card = ui::card(
            column![bind_field, ui::v_space(16.0), port_field].spacing(0),
        );

        let discovery_card = ui::card(
            row![
                column![
                    text("LAN discovery").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text(
                        "Advertise this server over mDNS so clients can find it."
                    ),
                ]
                .width(Length::Fill),
                checkbox(self.discoverable).on_toggle(Message::DiscoverableToggled),
            ]
            .align_y(iced::Alignment::Center),
        );

        let body = column![bind_card, ui::v_space(16.0), discovery_card].spacing(0);
        ui::page_body("Network", body)
    }

    fn security_page(&self) -> Element<'_, Message> {
        let auth_card = ui::card(
            row![
                column![
                    text("Require authentication").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text(
                        "Clients must present a passphrase before sending input."
                    ),
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
                ui::helper_text("Shared secret required when authentication is enabled."),
                ui::v_space(16.0),
                text_input("Set a passphrase", &self.passphrase)
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
