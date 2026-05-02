use iced::futures::Stream;
use iced::futures::StreamExt;
use iced::widget::{column, container, row, scrollable, stack};
use iced::{Background, Element, Length, Size, Subscription, Task, Theme};

use crate::components as ui;
use crate::config::{Config, Mode};
use crate::icons;
use crate::input::WaylandHandles;
use crate::theme as mt;
use crate::views::{client, server};

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
}

pub struct Spud {
    mode: Mode,
    showing_about: bool,
    client: client::State,
    server: server::State,
    window_size: Size,
    wayland_handles: Option<WaylandHandles>,
    handles_attempted: bool,
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
}

impl Spud {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let before = self.current_config();
        let task = self.handle(message);
        let after = self.current_config();
        if before != after {
            after.save();
        }
        task
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
                self.client.update(msg);
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
                Task::none()
            }
            Message::WindowOpened(id) => {
                if self.handles_attempted {
                    return Task::none();
                }
                self.handles_attempted = true;
                iced::window::run(id, |window| crate::input::extract_wayland_handles(window))
                    .map(Message::WaylandHandlesReady)
            }
            Message::WaylandHandlesReady(handles) => {
                self.wayland_handles = handles;
                Task::none()
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let resize = iced::window::resize_events().map(|(_, size)| Message::WindowResized(size));
        let mut subs = vec![resize];

        if !self.handles_attempted {
            subs.push(iced::window::open_events().map(Message::WindowOpened));
        }

        if self.mode == Mode::Client && self.client.hotkey_dialog_open {
            let keys = iced::keyboard::listen().filter_map(|event| {
                if let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    Some(Message::Client(client::Message::HotkeyInput(key, modifiers)))
                } else {
                    None
                }
            });
            subs.push(keys);
        } else if self.mode == Mode::Client && self.client.is_capturing_focused() {
            let capture = iced::event::listen()
                .map(|event| Message::Client(client::Message::Capture(event)));
            subs.push(capture);
        } else if self.mode == Mode::Client && self.client.is_capturing_hotkey() {
            let hotkey = self.client.hotkey_string().to_string();
            if let Some(handles) = self.wayland_handles {
                subs.push(Subscription::run_with(
                    handles,
                    build_wayland_hotkey_stream,
                ));
                let keyboard = iced::event::listen().filter_map(|event| match event {
                    iced::Event::Keyboard(_) => {
                        Some(Message::Client(client::Message::Capture(event)))
                    }
                    _ => None,
                });
                subs.push(keyboard);
            } else {
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
                    .view_content(card_inner_width)
                    .map(Message::Client),
                Mode::Server => self
                    .server
                    .view_content(card_inner_width)
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
            handle_overlay.into(),
        ];

        if let Some(dialog) = self.client.hotkey_dialog().map(|d| d.map(Message::Client)) {
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
}
