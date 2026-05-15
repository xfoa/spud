use iced::futures::Stream;
use iced::futures::StreamExt;
use iced::widget::{column, container, mouse_area, row, scrollable, stack, text};
use iced::{Background, Element, Length, Size, Subscription, Task, Theme};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::components as ui;
use crate::config::{Config, Mode};
use crate::icons;
use crate::input::WaylandHandles;
use crate::theme as mt;
use crate::views::{client, server};

fn build_discovery_stream(_: &()) -> impl Stream<Item = Message> + 'static {
    crate::discovery::browse()
        .map(|event| Message::Client(client::Message::DiscoveryEvent(event)))
}

fn build_net_events_stream(_: &()) -> impl Stream<Item = Message> + 'static {
    crate::net::events().map(|event| match event {
        crate::net::NetEvent::Disconnected => {
            Message::Client(client::Message::ConnectionLost)
        }
    })
}

fn build_hotkey_stream(hotkey: &String) -> impl Stream<Item = Message> + 'static {
    crate::input::listen(hotkey.clone())
        .map(|event| Message::Client(client::Message::HotkeyEvent(event)))
}

fn build_wayland_hotkey_stream(
    handles: &WaylandHandles,
) -> impl Stream<Item = Message> + 'static {
    crate::input::listen_wayland(*handles)
        .map(|event| Message::Client(client::Message::HotkeyEvent(event)))
}

fn build_signal_stream(_: &()) -> impl Stream<Item = Message> + 'static {
    iced::stream::channel(1, |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => return,
            };
            tokio::select! {
                _ = sigterm.recv() => {},
                _ = sigint.recv() => {},
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        let _ = output.try_send(Message::ForceQuit);
    })
}

async fn reconnect(
    host: String,
    port: u16,
    addrs: Option<Vec<std::net::SocketAddr>>,
    client_require_auth: bool,
    passphrase: Option<String>,
    saved_phc: Option<String>,
    client_encrypt: bool,
    timeout: std::time::Duration,
    cancel: Arc<AtomicBool>,
    max_batch: u8,
    udp_drop_percent: u8,
    batch_redundancy: u8,
) -> Result<(crate::net::Sender, Option<String>), ()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return Err(());
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match tokio::time::timeout(
            remaining,
            crate::net::Sender::connect(&host, port, addrs.clone(), client_encrypt, client_require_auth, passphrase.clone(), saved_phc.clone(), None, max_batch, udp_drop_percent, batch_redundancy),
        ).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(crate::net::client::ConnectError::FingerprintMismatch(_))) => return Err(()),
            Ok(Err(_)) => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Err(_) => return Err(()),
        }
    }
    Err(())
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchMode(Mode),
    ShowAbout,
    OpenUrl(String),
    Client(client::Message),
    Server(server::Message),
    StartResize,
    WindowResized(Size),
    WindowOpened(iced::window::Id),
    WaylandHandlesReady(Option<WaylandHandles>),
    Quit,
    ForceQuit,
    NoOp,
}

pub struct Spud {
    mode: Mode,
    showing_about: bool,
    client: client::State,
    server: server::State,
    window_size: Size,
    wayland_handles: Option<WaylandHandles>,
    handles_attempted: bool,
    window_id: Option<iced::window::Id>,
    blank_screen_fullscreen: bool,
}

impl Default for Spud {
    fn default() -> Self {
        let config = Config::load();
        Self {
            mode: config.mode,
            showing_about: false,
            client: client::State::from_config(&config.client),
            server: server::State::from_config(&config.server),
            window_size: Size::new(1000.0, 650.0),
            wayland_handles: None,
            handles_attempted: false,
            window_id: None,
            blank_screen_fullscreen: false,
        }
    }
}

impl Spud {
    fn current_config(&self) -> Config {
        Config {
            mode: self.mode,
            client: self.client.to_config(),
            server: self.server.to_config(),
        }
    }

    fn sync_blank_screen(&mut self) -> Task<Message> {
        let Some(id) = self.window_id else {
            return Task::none();
        };
        let should_be_fullscreen =
            self.mode == Mode::Client
                && self.client.capture_mode() == crate::config::CaptureMode::Fullscreen
                && self.client.is_blank_screen_active();
        if should_be_fullscreen == self.blank_screen_fullscreen {
            return Task::none();
        } else {
            self.blank_screen_fullscreen = should_be_fullscreen;
            let mode = if should_be_fullscreen {
                iced::window::Mode::Fullscreen
            } else {
                iced::window::Mode::Windowed
            };
            iced::window::set_mode(id, mode)
        }
    }
}

impl Spud {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let before = self.current_config();
        let task = self.handle(message);
        let after = self.current_config();
        if before != after {
            after.save();
        }
        let sync_task = self.sync_blank_screen();
        Task::batch(vec![task, sync_task])
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchMode(mode) => {
                self.mode = mode;
                Task::none()
            }
            Message::ShowAbout => {
                self.showing_about = true;
                Task::none()
            }
            Message::OpenUrl(url) => {
                let _ = open::that(url);
                Task::none()
            }
            Message::Client(msg) => {
                if matches!(msg, client::Message::SelectPage(_)) {
                    self.showing_about = false;
                }
                if let client::Message::DiscoveryEvent(crate::discovery::Event::Found(ref s)) = msg {
                    if self.server.owns_fullname(&s.fullname) {
                        return Task::none();
                    }
                }
                let is_connect = matches!(msg, client::Message::Connect);
                let was_lost = matches!(msg, client::Message::ConnectionLost);
                let fp_allow = if let client::Message::FingerprintDialogAllowOnce { host, port, new_fingerprint } = &msg {
                    Some((host.clone(), *port, *new_fingerprint))
                } else { None };
                let fp_trust = if let client::Message::FingerprintDialogTrust { host, port, new_fingerprint } = &msg {
                    Some((host.clone(), *port, *new_fingerprint))
                } else { None };
                self.client.update(msg);
                if is_connect {
                    let host = self.client.host().to_string();
                    let port = self.client.port().parse().unwrap_or(7878);
                    let addrs: Option<Vec<std::net::SocketAddr>> = {
                        let a = self.client.selected_addrs();
                        if a.is_empty() { None } else { Some(a.to_vec()) }
                    };
                    let client_require_auth = self.client.require_auth();
                    let passphrase = self.client.connection_passphrase().map(|s| s.to_string());
                    let saved_phc = if client_require_auth {
                        let phc = self.client.passphrase_hash().to_string();
                        if phc.is_empty() { None } else { Some(phc) }
                    } else {
                        None
                    };
                    let client_encrypt = self.client.encrypt_udp();
                    let max_batch = self.client.mouse_batch_size();
                    let udp_drop_percent = self.client.udp_drop_percent();
                    let batch_redundancy = self.client.batch_redundancy();
                    let host2 = host.clone();
                    return Task::perform(
                        async move {
                            crate::net::Sender::connect(&host2, port, addrs, client_encrypt, client_require_auth, passphrase, saved_phc, None, max_batch, udp_drop_percent, batch_redundancy).await
                        },
                        move |result| match result {
                            Ok((sender, phc)) => Message::Client(client::Message::ConnectSuccess(sender, phc)),
                            Err(crate::net::client::ConnectError::FingerprintMismatch(fp)) => {
                                Message::Client(client::Message::FingerprintMismatch { host: host.clone(), port, new_fingerprint: fp })
                            }
                            Err(e) => Message::Client(client::Message::ConnectFailed(format!("{e}"))),
                        },
                    );
                }
                if let Some((host, port, fp)) = fp_allow {
                    let addrs: Option<Vec<std::net::SocketAddr>> = {
                        let a = self.client.selected_addrs();
                        if a.is_empty() { None } else { Some(a.to_vec()) }
                    };
                    let client_require_auth = self.client.require_auth();
                    let passphrase = self.client.connection_passphrase().map(|s| s.to_string());
                    let saved_phc = if client_require_auth {
                        let phc = self.client.passphrase_hash().to_string();
                        if phc.is_empty() { None } else { Some(phc) }
                    } else {
                        None
                    };
                    let client_encrypt = self.client.encrypt_udp();
                    let max_batch = self.client.mouse_batch_size();
                    let udp_drop_percent = self.client.udp_drop_percent();
                    let batch_redundancy = self.client.batch_redundancy();
                    let host2 = host.clone();
                    return Task::perform(
                        async move {
                            crate::net::Sender::connect(&host2, port, addrs, client_encrypt, client_require_auth, passphrase, saved_phc, Some(fp), max_batch, udp_drop_percent, batch_redundancy).await
                        },
                        move |result| match result {
                            Ok((sender, phc)) => Message::Client(client::Message::ConnectSuccess(sender, phc)),
                            Err(crate::net::client::ConnectError::FingerprintMismatch(fp)) => {
                                Message::Client(client::Message::FingerprintMismatch { host: host.clone(), port, new_fingerprint: fp })
                            }
                            Err(e) => Message::Client(client::Message::ConnectFailed(format!("{e}"))),
                        },
                    );
                }
                if let Some((host, port, fp)) = fp_trust {
                    crate::config::trust_server(&host, port, fp);
                    let addrs: Option<Vec<std::net::SocketAddr>> = {
                        let a = self.client.selected_addrs();
                        if a.is_empty() { None } else { Some(a.to_vec()) }
                    };
                    let client_require_auth = self.client.require_auth();
                    let passphrase = self.client.connection_passphrase().map(|s| s.to_string());
                    let saved_phc = if client_require_auth {
                        let phc = self.client.passphrase_hash().to_string();
                        if phc.is_empty() { None } else { Some(phc) }
                    } else {
                        None
                    };
                    let client_encrypt = self.client.encrypt_udp();
                    let max_batch = self.client.mouse_batch_size();
                    let udp_drop_percent = self.client.udp_drop_percent();
                    let batch_redundancy = self.client.batch_redundancy();
                    let host2 = host.clone();
                    return Task::perform(
                        async move {
                            crate::net::Sender::connect(&host2, port, addrs, client_encrypt, client_require_auth, passphrase, saved_phc, None, max_batch, udp_drop_percent, batch_redundancy).await
                        },
                        move |result| match result {
                            Ok((sender, phc)) => Message::Client(client::Message::ConnectSuccess(sender, phc)),
                            Err(crate::net::client::ConnectError::FingerprintMismatch(fp)) => {
                                Message::Client(client::Message::FingerprintMismatch { host: host.clone(), port, new_fingerprint: fp })
                            }
                            Err(e) => Message::Client(client::Message::ConnectFailed(format!("{e}"))),
                        },
                    );
                }
                if was_lost && self.client.is_reconnecting() {
                    let host = self.client.host().to_string();
                    let port = self.client.port().parse().unwrap_or(7878);
                    let addrs: Option<Vec<std::net::SocketAddr>> = {
                        let a = self.client.selected_addrs();
                        if a.is_empty() { None } else { Some(a.to_vec()) }
                    };
                    let gen = self.client.reconnect_generation();
                    let timeout = self.client.reconnect_timeout();
                    let client_require_auth = self.client.require_auth();
                    let passphrase = self.client.connection_passphrase().map(|s| s.to_string());
                    let saved_phc = if client_require_auth {
                        let phc = self.client.passphrase_hash().to_string();
                        if phc.is_empty() { None } else { Some(phc) }
                    } else {
                        None
                    };
                    let client_encrypt = self.client.encrypt_udp();
                    let cancel = Arc::new(AtomicBool::new(false));
                    self.client.set_reconnect_cancel(cancel.clone());
                    return Task::perform(
                        reconnect(
                            host,
                            port,
                            addrs,
                            client_require_auth,
                            passphrase,
                            saved_phc,
                            client_encrypt,
                            timeout,
                            cancel,
                            self.client.mouse_batch_size(),
                            self.client.udp_drop_percent(),
                            self.client.batch_redundancy(),
                        ),
                        move |result| match result {
                            Ok((sender, _phc)) => {
                                Message::Client(client::Message::ReconnectSuccess(sender, gen))
                            }
                            Err(()) => Message::Client(client::Message::ReconnectFailed(gen)),
                        },
                    );
                }
                Task::none()
            }
            Message::Server(msg) => {
                if matches!(msg, server::Message::SelectPage(_)) {
                    self.showing_about = false;
                }
                self.server.update(msg);
                Task::none()
            }
            Message::StartResize => iced::window::latest().and_then(|id| {
                iced::window::drag_resize(id, iced::window::Direction::SouthEast)
            }),
            Message::WindowResized(size) => {
                self.window_size = size;
                self.client.update(client::Message::WindowSizeChanged(size));
                Task::none()
            }
            Message::WindowOpened(id) => {
                if self.handles_attempted {
                    return Task::none();
                }
                self.handles_attempted = true;
                self.window_id = Some(id);
                iced::window::run(id, |window| crate::input::extract_wayland_handles(window))
                    .map(Message::WaylandHandlesReady)
            }
            Message::WaylandHandlesReady(handles) => {
                if handles.is_some() {
                    eprintln!("[spud] Wayland handles acquired");
                } else {
                    eprintln!("[spud] Wayland handles not available, falling back to X11 backend");
                }
                self.wayland_handles = handles;
                Task::none()
            }
            Message::ForceQuit => {
                return iced::exit();
            }
            Message::Quit => {
                let can_quit = if !self.client.is_connected() {
                    true
                } else if self.client.capture_mode() == crate::config::CaptureMode::Fullscreen {
                    !crate::input::is_wayland_grabbed()
                } else {
                    !self.client.is_grabbed()
                };
                if can_quit {
                    return iced::exit();
                }
                Task::none()
            }
            Message::NoOp => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let resize = iced::window::resize_events().map(|(_, size)| Message::WindowResized(size));
        let mut subs = vec![resize];

        if !self.handles_attempted {
            subs.push(iced::window::open_events().map(Message::WindowOpened));
        }

        subs.push(Subscription::run_with((), build_discovery_stream));
        subs.push(Subscription::run_with((), build_net_events_stream));
        subs.push(Subscription::run_with((), build_signal_stream));

        if self.client.is_connected() {
            subs.push(
                iced::time::every(self.client.keyrepeat_interval())
                    .map(|_| Message::Client(client::Message::KeyRepeatTick)),
            );
            subs.push(
                iced::time::every(self.client.keepalive_interval())
                    .map(|_| Message::Client(client::Message::KeepaliveTick)),
            );
        }
        subs.push(iced::keyboard::listen().filter_map(|event| {
            if let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                if modifiers.control() && key == iced::keyboard::Key::Character("q".into()) {
                    return Some(Message::Quit);
                }
            }
            None
        }));

        if self.mode == Mode::Client && self.client.hotkey_dialog_open {
            let keys = iced::keyboard::listen().filter_map(|event| {
                if let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    Some(Message::Client(client::Message::HotkeyInput(key, modifiers)))
                } else {
                    None
                }
            });
            subs.push(keys);
        } else if self.mode == Mode::Client && self.client.is_capturing_window() {
            let capture = iced::event::listen_raw(|event, _status, _window| {
                if matches!(event, iced::Event::Window(iced::window::Event::RedrawRequested(_))) {
                    None
                } else {
                    Some(Message::Client(client::Message::Capture(event)))
                }
            });
            subs.push(capture);
        } else if self.mode == Mode::Client && self.client.is_capturing_fullscreen() {
            // Always subscribe to iced keyboard events so the Capture handler
            // can update user_capturing on both X11 and Wayland.
            let keyboard = iced::event::listen().filter_map(|event| match event {
                iced::Event::Keyboard(_) => {
                    Some(Message::Client(client::Message::Capture(event)))
                }
                _ => None,
            });
            subs.push(keyboard);

            if let Some(handles) = self.wayland_handles {
                subs.push(Subscription::run_with(handles, build_wayland_hotkey_stream));
            } else {
                let hotkey = self.client.hotkey_string().to_string();
                subs.push(Subscription::run_with(hotkey, build_hotkey_stream));
            }
        }

        Subscription::batch(subs)
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Top tab bar
        let tabs = row![
            ui::top_tab("Client", self.mode == Mode::Client, Message::SwitchMode(Mode::Client)),
            ui::top_tab("Server", self.mode == Mode::Server, Message::SwitchMode(Mode::Server)),
        ]
        .spacing(0)
        .width(Length::Fill);

        let app_bar = container(tabs)
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(mt::SURFACE)),
                ..Default::default()
            });

        // Sidebar: mode-specific items on top, shared About pinned at bottom
        let mode_nav: Vec<Element<Message>> = match self.mode {
            Mode::Client => self
                .client
                .nav_items(self.showing_about)
                .into_iter()
                .map(|e| e.map(Message::Client))
                .collect(),
            Mode::Server => self
                .server
                .nav_items(self.showing_about)
                .into_iter()
                .map(|e| e.map(Message::Server))
                .collect(),
        };

        let nav_top = column(mode_nav).spacing(4).width(Length::Fill);

        let about_btn = ui::nav_item("About", icons::CIRCLE_INFO, self.showing_about, Message::ShowAbout);

        let nav_col = column![nav_top, ui::v_space_fill(), about_btn]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill);

        let sidebar = ui::sidebar(nav_col);

        // Available width inside a card on the content page:
        // window - sidebar(232) - page_body padding(32 each side) - card padding(20 each side)
        let card_inner_width = (self.window_size.width - 232.0 - 64.0 - 40.0).max(0.0);

        // Content area
        let content: Element<Message> = if self.showing_about {
            ui::about_page(Message::OpenUrl)
        } else {
            match self.mode {
                Mode::Client => self
                    .client
                    .view_content(card_inner_width, self.server.is_running())
                    .map(Message::Client),
                Mode::Server => self
                    .server
                    .view_content(card_inner_width, self.client.is_connected())
                    .map(Message::Server),
            }
        };

        let body = row![
            sidebar,
            scrollable(content).width(Length::Fill).height(Length::Fill),
        ]
        .height(Length::Fill)
        .width(Length::Fill);

        let body_container = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(mt::BACKGROUND)),
                ..Default::default()
            });

        let window_content = column![app_bar, body_container]
            .width(Length::Fill)
            .height(Length::Fill);

        let handle_overlay = container(ui::corner_handle(Message::StartResize))
            .align_right(Length::Fill)
            .align_bottom(Length::Fill);

        let mut layers: Vec<Element<Message>> = vec![
            window_content.into(),
        ];

        if self.mode == Mode::Client && self.client.is_capturing_fullscreen() {
            let show_text = self.client.is_blank_screen_active() && self.client.show_hotkey_on_blank();
            let overlay_text: Element<Message> = if show_text {
                text(String::new() + "Press " + self.client.hotkey_display() + " to stop capturing")
                    .size(24)
                    .color(iced::Color::from_rgb(0.25, 0.25, 0.25))
                    .into()
            } else {
                text("").size(1).into()
            };
            let bg = if self.client.is_blank_screen_active() {
                iced::Color::BLACK
            } else {
                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.0)
            };
            let mut overlay = mouse_area(
                container(overlay_text)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(bg)),
                        ..Default::default()
                    }),
            );
            if self.client.is_grabbed() {
                overlay = overlay.interaction(iced::mouse::Interaction::Hidden);
            }
            layers.push(overlay.into());
        }

        if self.mode == Mode::Client && self.client.is_capturing_window() && self.client.is_grabbed() {
            let show_text = self.client.show_hotkey_on_blank();
            let overlay_text: Element<Message> = if show_text {
                text(String::new() + "Press " + self.client.hotkey_display() + " to stop capturing")
                    .size(24)
                    .color(iced::Color::from_rgb(0.25, 0.25, 0.25))
                    .into()
            } else {
                text("").size(1).into()
            };
            let overlay = mouse_area(
                container(overlay_text)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .style(|_| container::Style {
                        background: Some(Background::Color(iced::Color::BLACK)),
                        ..Default::default()
                    }),
            )
            .on_press(Message::NoOp)
            .on_double_click(Message::NoOp)
            .on_right_press(Message::NoOp)
            .on_middle_press(Message::NoOp)
            .on_scroll(|_| Message::NoOp)
            .interaction(iced::mouse::Interaction::Idle)
            .into();
            layers.push(overlay);
        }

        if self.mode == Mode::Server {
            if let Some(banner) = self.server.restart_banner() {
                let overlay = container(banner.map(Message::Server))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Bottom)
                    .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 232.0 })
                    .into();
                layers.push(overlay);
            }
        }

        layers.push(handle_overlay.into());

        if let Some(dialog) = self.client.hotkey_dialog().map(|d| d.map(Message::Client)) {
            layers.push(dialog);
        }

        if let Some(dialog) = self.client.fingerprint_dialog().map(|d| d.map(Message::Client)) {
            layers.push(dialog);
        }

        stack(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        mt::material_theme()
    }

    pub fn title(&self) -> String {
        if self.mode == Mode::Client && self.client.is_grabbed() {
            format!("Spud - Capturing (press {} to stop)", self.client.hotkey_display())
        } else {
            "Spud".to_string()
        }
    }
}
