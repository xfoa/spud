use iced::widget::{checkbox, column, row, slider, text, text_input, Row};
use iced::{Element, Length};

use crate::components as ui;
use crate::config::{hash_passphrase, ServerConfig, ServerIcon};
use crate::icons;
use crate::theme as mt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Status,
    Network,
    Security,
    Advanced,
}

impl Page {
    const ALL: [Page; 4] = [Page::Status, Page::Network, Page::Security, Page::Advanced];

    fn label(self) -> &'static str {
        match self {
            Page::Status => "Status",
            Page::Network => "Network",
            Page::Security => "Security",
            Page::Advanced => "Advanced",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Status => icons::SIGNAL,
            Page::Network => icons::NETWORK_WIRED,
            Page::Security => icons::SHIELD_HALVED,
            Page::Advanced => icons::GEAR,
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
    IconChanged(ServerIcon),
    NameChanged(String),
    KeyTimeoutChanged(u16),
    EncryptUdpToggled(bool),
}

#[derive(Clone, PartialEq)]
struct RunningConfig {
    bind_address: String,
    port: String,
    discoverable: bool,
    require_auth: bool,
    passphrase_hash: String,
    name: String,
    icon: ServerIcon,
    key_timeout_ms: u16,
    encrypt_udp: bool,
}

pub struct State {
    page: Page,
    running: bool,
    bind_address: String,
    bind_options: Vec<String>,
    port: String,
    discoverable: bool,
    require_auth: bool,
    passphrase: String,
    passphrase_hash: String,
    icon: ServerIcon,
    name: String,
    active_config: Option<RunningConfig>,
    registration: Option<crate::discovery::Registration>,
    listener: Option<crate::net::Listener>,
    key_timeout_ms: u16,
    last_error: Option<String>,
    encrypt_udp: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::from_config(&ServerConfig::default())
    }
}

impl State {
    pub fn from_config(cfg: &ServerConfig) -> Self {
        let bind_address = if cfg.bind_address.is_empty() {
            "0.0.0.0".to_string()
        } else {
            cfg.bind_address.clone()
        };
        let mut bind_options = crate::discovery::bind_options();
        if !bind_options.contains(&bind_address) {
            bind_options.push(bind_address.clone());
        }
        Self {
            page: Page::Status,
            running: false,
            bind_address,
            bind_options,
            port: cfg.port.clone(),
            discoverable: cfg.discoverable,
            require_auth: cfg.require_auth,
            passphrase: String::new(),
            passphrase_hash: cfg.passphrase_hash.clone(),
            icon: cfg.icon,
            name: cfg.name.clone(),
            key_timeout_ms: cfg.key_timeout_ms,
            active_config: None,
            registration: None,
            listener: None,
            last_error: None,
            encrypt_udp: cfg.encrypt_udp,
        }
    }

    pub fn to_config(&self) -> ServerConfig {
        ServerConfig {
            name: self.name.clone(),
            icon: self.icon,
            bind_address: self.bind_address.clone(),
            port: self.port.clone(),
            discoverable: self.discoverable,
            require_auth: self.require_auth,
            passphrase_hash: self.passphrase_hash.clone(),
            key_timeout_ms: self.key_timeout_ms,
            encrypt_udp: self.encrypt_udp,
        }
    }
}

impl State {
    fn snapshot(&self) -> RunningConfig {
        RunningConfig {
            bind_address: self.bind_address.clone(),
            port: self.port.clone(),
            discoverable: self.discoverable,
            require_auth: self.require_auth,
            passphrase_hash: self.passphrase_hash.clone(),
            name: self.name.clone(),
            icon: self.icon,
            key_timeout_ms: self.key_timeout_ms,
            encrypt_udp: self.encrypt_udp,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    fn settings_changed(&self) -> bool {
        if !self.running {
            return false;
        }
        let config_changed = self.active_config.as_ref().is_some_and(|c| *c != self.snapshot());
        let passphrase_pending = !self.passphrase.is_empty();
        config_changed || passphrase_pending
    }

    pub fn owns_fullname(&self, fullname: &str) -> bool {
        self.registration
            .as_ref()
            .map_or(false, |r| r.fullname() == fullname)
    }

    fn refresh_registration(&mut self) {
        self.registration = None;
        if self.running && self.discoverable {
            let port = self.port.parse::<u16>().unwrap_or(7878);
            self.registration = crate::discovery::Registration::new(&self.name, port, &self.bind_address, self.icon, self.require_auth, self.encrypt_udp);
        }
    }

    fn start_listener(&mut self) -> std::io::Result<()> {
        let port = self.port.parse::<u16>().unwrap_or(7878);
        let addr = if self.bind_address.is_empty() {
            "0.0.0.0"
        } else {
            self.bind_address.as_str()
        };
        let listener = tokio::runtime::Handle::current().block_on(
            crate::net::Listener::bind(
                addr,
                port,
                self.key_timeout_ms,
                self.require_auth,
                self.passphrase_hash.clone(),
                self.encrypt_udp,
            )
        )?;
        self.listener = Some(listener);
        Ok(())
    }
}

impl State {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectPage(p) => {
                if self.page == Page::Security && p != Page::Security && !self.passphrase.is_empty() {
                    self.passphrase_hash = hash_passphrase(&self.passphrase);
                    self.passphrase.clear();
                }
                self.page = p;
            }
            Message::StartServer => match self.start_listener() {
                Ok(()) => {
                    self.running = true;
                    self.active_config = Some(self.snapshot());
                    self.refresh_registration();
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(format!("{e}"));
                }
            },
            Message::StopServer => {
                self.running = false;
                self.active_config = None;
                self.registration = None;
                self.listener = None;
                self.last_error = None;
            }
            Message::RestartServer => {
                self.listener = None;
                match self.start_listener() {
                    Ok(()) => {
                        self.active_config = Some(self.snapshot());
                        self.refresh_registration();
                        self.last_error = None;
                    }
                    Err(e) => {
                        self.running = false;
                        self.active_config = None;
                        self.registration = None;
                        self.last_error = Some(format!("{e}"));
                    }
                }
            }
            Message::BindAddressChanged(s) => self.bind_address = s,
            Message::PortChanged(s) => {
                if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5 {
                    self.port = s;
                }
            }
            Message::DiscoverableToggled(v) => {
                self.discoverable = v;
                if self.running {
                    self.refresh_registration();
                }
            }
            Message::RequireAuthToggled(v) => self.require_auth = v,
            Message::PassphraseChanged(s) => self.passphrase = s,
            Message::IconChanged(c) => self.icon = c,
            Message::NameChanged(s) => self.name = s,
            Message::KeyTimeoutChanged(v) => {
                self.key_timeout_ms = (v / 50) * 50;
            }
            Message::EncryptUdpToggled(v) => self.encrypt_udp = v,
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

    pub fn view_content(&self, _content_width: f32, client_connected: bool) -> Element<'_, Message> {
        match self.page {
            Page::Status => self.status_page(client_connected),
            Page::Network => self.network_page(),
            Page::Security => self.security_page(),
            Page::Advanced => self.advanced_page(),
        }
    }

    pub fn restart_banner(&self) -> Option<Element<'_, Message>> {
        if !self.settings_changed() {
            return None;
        }
        let content: Element<Message> = row![
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
        .into();
        let styled = iced::widget::container(content)
            .padding(12)
            .width(iced::Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(mt::WARNING_CONTAINER)),
                border: iced::Border {
                    color: mt::WARNING,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });
        Some(
            iced::widget::container(styled)
                .padding(16)
                .width(iced::Length::Fill)
                .into(),
        )
    }

    fn status_page(&self, client_connected: bool) -> Element<'_, Message> {
        let active_require_auth = self
            .active_config
            .as_ref()
            .map_or(self.require_auth, |c| c.require_auth);

        let active_encryption_enabled = self
            .active_config
            .as_ref()
            .map_or(self.encrypt_udp, |c| c.encrypt_udp);

        let (status_label, status_color, lock_icon) = if self.running {
            let running_label = String::from("Listening");
            if active_require_auth && active_encryption_enabled {
                (running_label, mt::SUCCESS, Some(icons::LOCK))
            } else {
                let mut disabled_features = vec![];
                    if !active_encryption_enabled {
                    disabled_features.push("encryption disabled");
                }
                if !active_require_auth {
                    disabled_features.push("no passphrase required");
                }
                let warning_label = String::from(running_label) + " (" + &disabled_features.join(", ") + ")";
                (warning_label, mt::DANGER, Some(icons::TRIANGLE_EXCLAMATION))
            }
        } else {
            (String::from("Stopped"), mt::ON_SURFACE_VARIANT, None)
        };

        let passphrase_missing = self.require_auth && self.passphrase.is_empty() && self.passphrase_hash.is_empty();

        let action: Element<Message> = if self.running {
            ui::outlined_button("Stop server", Message::StopServer)
        } else {
            ui::filled_button("Start server", (!passphrase_missing && !client_connected).then_some(Message::StartServer))
        };

        let port = self.port.parse::<u16>().unwrap_or(7878);
        let endpoint = self.active_config.as_ref().map_or_else(
            || crate::discovery::display_endpoint(&self.bind_address, port),
            |c| crate::discovery::display_endpoint(&c.bind_address, port),
        );

        let mut status_row: Row<Message> =
            row![
                text("Status:").size(14).color(mt::ON_SURFACE_VARIANT)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into();

        if let Some(icon) = lock_icon { 
            status_row = status_row.push(text(icon).font(icons::FA_SOLID).size(13).color(status_color));
        }

        status_row = status_row.push(text(status_label).size(14).color(status_color));

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

        let mut col_items: Vec<Element<Message>> = vec![status_row.into()];

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

        if client_connected && !self.running {
            col_items.push(ui::v_space(12.0).into());
            col_items.push(ui::divider().into());
            col_items.push(ui::v_space(12.0).into());
            col_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::WARNING),
                    text("Disconnect the client before starting the server.")
                        .size(13)
                        .color(mt::WARNING),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        } else if passphrase_missing {
            col_items.push(ui::v_space(12.0).into());
            col_items.push(ui::divider().into());
            col_items.push(ui::v_space(12.0).into());
            col_items.push(
                row![
                    text(icons::TRIANGLE_EXCLAMATION)
                        .font(icons::FA_SOLID)
                        .size(13)
                        .color(mt::WARNING),
                    text("Set a passphrase in Security settings before starting the server.")
                        .size(13)
                        .color(mt::WARNING),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        if let Some(err) = &self.last_error {
            col_items.push(ui::v_space(12.0).into());
            col_items.push(ui::divider().into());
            col_items.push(ui::v_space(12.0).into());
            col_items.push(
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
            ServerIcon::ALL
                .iter()
                .copied()
                .map(|ic| {
                    ui::icon_pick(ic.glyph(), ic == self.icon, Message::IconChanged(ic))
                })
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
            ui::pick_list(
                self.bind_options.clone(),
                Some(self.bind_address.clone()),
                Message::BindAddressChanged,
            ),
            ui::v_space(4.0),
            ui::helper_text("0.0.0.0 binds to all interfaces and advertises all detected IPs."),
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
        let mut auth_items: Vec<Element<Message>> = vec![
            row![
                column![
                    text("Require passphrase").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text(
                        "Clients must present a passphrase before sending input."
                    ),
                ]
                .width(Length::Fill),
                checkbox(self.require_auth).on_toggle(Message::RequireAuthToggled),
            ]
            .align_y(iced::Alignment::Center)
            .into(),
        ];

        if self.require_auth {
            auth_items.push(ui::v_space(16.0).into());
            auth_items.push(text("Passphrase").size(16).color(mt::ON_SURFACE).into());
            auth_items.push(ui::v_space(4.0).into());
            auth_items.push(
                ui::helper_text("Shared secret required when authentication is enabled.")
                    .into(),
            );
            auth_items.push(ui::v_space(16.0).into());
            auth_items.push(
                text_input("Set a passphrase", &self.passphrase)
                    .on_input(Message::PassphraseChanged)
                    .secure(true)
                    .padding(12)
                    .size(14)
                    .into(),
            );

            if self.passphrase.is_empty() {
                if !self.passphrase_hash.is_empty() {
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

        let auth_card = ui::card(column(auth_items).spacing(0));

        let encrypt_card = ui::card(
            row![
                column![
                    text("Require encryption").size(16).color(mt::ON_SURFACE),
                    ui::v_space(2.0),
                    ui::helper_text("Encrypt input events sent over the network. Disabling this is less secure, but reduces latency."),
                ]
                .width(Length::Fill),
                checkbox(self.encrypt_udp).on_toggle(Message::EncryptUdpToggled),
            ]
            .align_y(iced::Alignment::Center),
        );

        let body = column![auth_card, ui::v_space(16.0), encrypt_card].spacing(0);
        ui::page_body("Security", body)
    }

    fn advanced_page(&self) -> Element<'_, Message> {
        let slider_row = row![
            slider(50..=2000, self.key_timeout_ms, Message::KeyTimeoutChanged)
                .width(Length::Fill),
            ui::h_space(12.0),
            text(format!("{} ms", self.key_timeout_ms)).size(14).color(mt::ON_SURFACE),
        ]
        .align_y(iced::Alignment::Center);

        let timeout_field = column![
            ui::field_label("Key timeout"),
            slider_row,
            ui::v_space(4.0),
            ui::helper_text(
                "Compensates for lost packets. Lower values are better for less reliable network conditions."
            ),
        ]
        .spacing(6);

        let timeout_card = ui::card(timeout_field);
        let body = column![timeout_card].spacing(0);
        ui::page_body("Advanced", body)
    }
}
