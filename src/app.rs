use iced::widget::{column, container, row, scrollable, stack};
use iced::{Background, Element, Length, Task, Theme};

use crate::components as ui;
use crate::icons;
use crate::theme as mt;
use crate::views::{client, server};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Client,
    Server,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchMode(Mode),
    ShowAbout,
    Client(client::Message),
    Server(server::Message),
    StartResize,
}

pub struct Spud {
    mode: Mode,
    showing_about: bool,
    client: client::State,
    server: server::State,
}

impl Default for Spud {
    fn default() -> Self {
        Self {
            mode: Mode::Client,
            showing_about: false,
            client: client::State::default(),
            server: server::State::default(),
        }
    }
}

impl Spud {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchMode(mode) => {
                self.mode = mode;
                Task::none()
            }
            Message::ShowAbout => {
                self.showing_about = true;
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
        }
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

        // Content area
        let content: Element<Message> = if self.showing_about {
            ui::about_page()
        } else {
            match self.mode {
                Mode::Client => self.client.view_content().map(Message::Client),
                Mode::Server => self.server.view_content().map(Message::Server),
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

        stack![window_content, handle_overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        mt::material_theme()
    }
}
