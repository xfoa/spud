use iced::widget::{checkbox, column, row, text, text_input};
use iced::{Element, Length};

use crate::components as ui;
use crate::icons;
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

    fn icon(self) -> char {
        match self {
            Page::Status => icons::SIGNAL,
            Page::Network => icons::NETWORK_WIRED,
            Page::Security => icons::SHIELD_HALVED,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectPage(Page),
    StartServer,
    StopServer,
    RestartServer,
    BindAddressChanged(String),
    PortChanged(String),
    DiscoverableToggled(bool),
    RequireAuthToggled(bool),
    PassphraseChanged(String),
    IconChanged(char),
    NameChanged(String),
}

pub const ICON_CHOICES: [char; 3] = [icons::DESKTOP, icons::LAPTOP, icons::SERVER];

#[derive(Clone, PartialEq)]
struct ServerConfig {
    bind_address: String,
    port: String,
    discoverable: bool,
    require_auth: bool,
    passphrase: String,
    name: String,
    icon: char,
}

fn default_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "spud-server".to_string())
}

pub struct State {
    page: Page,
    running: bool,
    bind_address: String,
    port: String,
    discoverable: bool,
    require_auth: bool,
    passphrase: String,
    icon: char,
    name: String,
    active_config: Option<ServerConfig>,
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
            icon: icons::DESKTOP,
            name: default_hostname(),
            active_config: None,
        }
    }
}

impl State {
    fn snapshot(&self) -> ServerConfig {
        ServerConfig {
            bind_address: self.bind_address.clone(),
            port: self.port.clone(),
            discoverable: self.discoverable,
            require_auth: self.require_auth,
            passphrase: self.passphrase.clone(),
            name: self.name.clone(),
            icon: self.icon,
        }
    }

    fn settings_changed(&self) -> bool {
        self.running
            && self.active_config.as_ref().is_some_and(|c| *c != self.snapshot())
    }
}

impl State {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectPage(p) => self.page = p,
            Message::StartServer => {
                self.running = true;
                self.active_config = Some(self.snapshot());
            }
            Message::StopServer => {
                self.running = false;
                self.active_config = None;
            }
            Message::RestartServer => {
                self.active_config = Some(self.snapshot());
            }
            Message::BindAddressChanged(s) => self.bind_address = s,
            Message::PortChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.port = s;
                }
            }
            Message::DiscoverableToggled(v) => self.discoverable = v,
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.passphrase = s,
            Message::IconChanged(c) => self.icon = c,
            Message::NameChanged(s) => self.name = s,
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

    pub fn view_content(&self, _content_width: f32) -> Element<'_, Message> {
        match self.page {
            Page::Status => self.status_page(),
            Page::Network => self.network_page(),
            Page::Security => self.security_page(),
        }
    }

    fn status_page(&self) -> Element<'_, Message> {
        let active_require_auth = self
            .active_config
            .as_ref()
            .map_or(self.require_auth, |c| c.require_auth);

        let (status_label, status_color, lock_icon) = if self.running {
            if active_require_auth {
                ("Listening", mt::SUCCESS, icons::LOCK)
            } else {
                ("Listening (insecure)", mt::DANGER, icons::TRIANGLE_EXCLAMATION)
            }
        } else {
            ("Stopped", mt::ON_SURFACE_VARIANT, icons::TRIANGLE_EXCLAMATION)
        };

        let action: Element<Message> = if self.running {
            ui::outlined_button("Stop server", Message::StopServer)
        } else {
            ui::filled_button("Start server", Some(Message::StartServer))
        };

        let endpoint = self.active_config.as_ref().map_or_else(
            || format!("{}:{}", self.bind_address, self.port),
            |c| format!("{}:{}", c.bind_address, c.port),
        );

        let status_row: Element<Message> = if self.running {
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT),
                text(lock_icon).font(icons::FA_SOLID).size(13).color(status_color),
                text(status_label).size(14).color(status_color),
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

        let action_row: Element<Message> = if self.settings_changed() {
            row![
                ui::h_space_fill(),
                ui::outlined_button("Stop server", Message::StopServer),
                ui::h_space(8.0),
                ui::filled_button("Restart server", Some(Message::RestartServer)),
            ]
            .width(Length::Fill)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            row![ui::h_space_fill(), action].width(Length::Fill).into()
        };

        let mut col_items: Vec<Element<Message>> = vec![status_row];

        if self.running {
            col_items.push(ui::v_space(8.0).into());
            col_items.push(
                row![
                    text("Listening on:").size(14).color(mt::ON_SURFACE_VARIANT),
                    text(endpoint).size(14).color(mt::ON_SURFACE),
                ]
                .spacing(8)
                .into(),
            );
        }

        if self.settings_changed() {
            col_items.push(ui::v_space(12.0).into());
            col_items.push(ui::divider().into());
            col_items.push(ui::v_space(12.0).into());
            col_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::WARNING),
                    text("Settings have changed - restart the server to apply them.")
                        .size(13)
                        .color(mt::WARNING),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        col_items.push(ui::v_space(20.0).into());
        col_items.push(action_row);

        let card_content = column(col_items).spacing(0);

        let identity_card = self.identity_card();
        let status_card = ui::card(card_content);

        let body = column![identity_card, ui::v_space(16.0), status_card].spacing(0);
        ui::page_body("Status", body)
    }

    fn identity_card(&self) -> Element<'_, Message> {
        let name_field = column![
            ui::field_label("Server name"),
            text_input("spud-server", &self.name)
                .on_input(Message::NameChanged)
                .padding(12)
                .size(14),
            ui::v_space(4.0),
            ui::helper_text("Shown to clients when they discover this server."),
        ]
        .spacing(6);

        let icon_picker_row = row(
            ICON_CHOICES
                .iter()
                .copied()
                .map(|c| ui::icon_pick(c, c == self.icon, Message::IconChanged(c)))
                .collect::<Vec<_>>(),
        )
        .spacing(10);

        let icon_section = column![
            ui::field_label("Icon"),
            ui::v_space(2.0),
            icon_picker_row,
        ]
        .spacing(0);

        ui::card(
            column![name_field, ui::v_space(16.0), icon_section].spacing(0),
        )
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
