use std::sync::LazyLock;

use iced::widget::text::Wrapping;
use iced::widget::tooltip;
use iced::widget::{button, column, container, image, mouse_area, row, stack, text, Space};
use iced::{font, Background, Border, Color, Element, Font, Length, Padding, Shadow, Vector};

use crate::theme as mt;

static ABOUT_ICON: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../resources/icon.png").as_slice())
});

pub fn v_space(h: f32) -> Space {
    Space::new().height(Length::Fixed(h))
}

pub fn h_space(w: f32) -> Space {
    Space::new().width(Length::Fixed(w))
}

pub fn h_space_fill() -> Space {
    Space::new().width(Length::Fill)
}

pub fn v_space_fill() -> Space {
    Space::new().height(Length::Fill)
}

pub fn top_tab<'a, Message: 'a + Clone>(
    label: &'a str,
    active: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let label_color = if active { mt::ON_SURFACE } else { mt::ON_SURFACE_VARIANT };
    let indicator_color = if active { mt::PRIMARY } else { Color::TRANSPARENT };

    let content = column![
        container(text(label).size(15).color(label_color))
            .center_x(Length::Fill)
            .padding(Padding::from([14, 24])),
        container(Space::new().height(Length::Fixed(4.0)))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(indicator_color)),
                ..Default::default()
            }),
    ];

    button(content)
        .on_press(on_press)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| {
            let base_bg = if active {
                mt::with_alpha(mt::PRIMARY_CONTAINER, 0.45)
            } else {
                Color::TRANSPARENT
            };
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    mt::with_alpha(mt::PRIMARY, 0.08)
                }
                _ => base_bg,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: label_color,
                border: Border::default(),
                shadow: Shadow::default(),
                ..Default::default()
            }
        })
        .into()
}

pub fn nav_item<'a, Message: 'a + Clone>(
    label: &'a str,
    icon: char,
    active: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let (text_color, icon_color) = if active {
        (mt::ON_PRIMARY_CONTAINER, mt::ON_PRIMARY_CONTAINER)
    } else {
        (mt::ON_SURFACE, mt::ON_SURFACE_VARIANT)
    };

    let icon_widget = container(
        text(icon)
            .font(crate::icons::FA_SOLID)
            .size(16)
            .color(icon_color),
    )
    .center_x(Length::Fixed(22.0));

    let content = row![icon_widget, text(label).size(14).color(text_color)]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    button(container(content).padding(Padding::from([10, 16])).width(Length::Fill))
        .on_press(on_press)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| {
            let base_bg = if active {
                mt::PRIMARY_CONTAINER
            } else {
                Color::TRANSPARENT
            };
            let bg = match status {
                button::Status::Hovered => {
                    if active {
                        mt::PRIMARY_CONTAINER
                    } else {
                        mt::with_alpha(mt::PRIMARY, 0.07)
                    }
                }
                button::Status::Pressed => mt::with_alpha(mt::PRIMARY, 0.12),
                _ => base_bg,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    radius: 999.0.into(),
                    ..Default::default()
                },
                shadow: Shadow::default(),
                ..Default::default()
            }
        })
        .into()
}

pub fn filled_button<'a, Message: 'a + Clone>(
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let disabled = on_press.is_none();
    let mut btn = button(
        container(text(label).size(14).color(if disabled {
            mt::with_alpha(mt::ON_PRIMARY, 0.4)
        } else {
            mt::ON_PRIMARY
        }))
        .padding(Padding::from([10, 24])),
    )
    .padding(0)
    .style(move |_, status| {
        let bg = if disabled {
            mt::with_alpha(mt::ON_SURFACE, 0.12)
        } else {
            match status {
                button::Status::Hovered => darken(mt::PRIMARY, 0.05),
                button::Status::Pressed => darken(mt::PRIMARY, 0.10),
                _ => mt::PRIMARY,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: mt::ON_PRIMARY,
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
            shadow: if disabled {
                Shadow::default()
            } else {
                Shadow {
                    color: mt::with_alpha(Color::BLACK, 0.15),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                }
            },
            ..Default::default()
        }
    });
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn.into()
}

pub fn outlined_button<'a, Message: 'a + Clone>(
    label: &'a str,
    on_press: Message,
) -> Element<'a, Message> {
    button(
        container(text(label).size(14).color(mt::ON_SURFACE_VARIANT))
            .padding(Padding::from([10, 24])),
    )
    .on_press(on_press)
    .padding(0)
    .style(|_, status| {
        let bg = match status {
            button::Status::Hovered => mt::with_alpha(mt::ON_SURFACE, 0.06),
            button::Status::Pressed => mt::with_alpha(mt::ON_SURFACE, 0.12),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: mt::ON_SURFACE_VARIANT,
            border: Border {
                color: mt::OUTLINE,
                width: 1.0,
                radius: 999.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    })
    .into()
}

pub fn pick_list<'a, T, L, V, Message>(
    options: L,
    selected: Option<V>,
    on_select: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    L: std::borrow::Borrow<[T]> + 'a,
    V: std::borrow::Borrow<T> + 'a,
    Message: 'a + Clone,
{
    use iced::overlay::menu;
    use iced::widget::pick_list::{self as pl, Status};

    iced::widget::pick_list(options, selected, on_select)
        .style(|_, status| pl::Style {
            text_color: mt::ON_SURFACE,
            placeholder_color: mt::ON_SURFACE_VARIANT,
            handle_color: mt::ON_SURFACE_VARIANT,
            background: mt::SURFACE.into(),
            border: Border {
                radius: 2.0.into(),
                width: 1.0,
                color: match status {
                    Status::Active => mt::OUTLINE_VARIANT,
                    _ => mt::PRIMARY,
                },
            },
        })
        .menu_style(|_| menu::Style {
            background: mt::SURFACE.into(),
            border: Border {
                radius: 2.0.into(),
                width: 1.0,
                color: mt::OUTLINE_VARIANT,
            },
            text_color: mt::ON_SURFACE,
            selected_text_color: mt::ON_PRIMARY_CONTAINER,
            selected_background: mt::PRIMARY_CONTAINER.into(),
            shadow: Shadow::default(),
        })
        .padding(10)
        .width(Length::Fill)
        .into()
}

pub fn card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(mt::SURFACE)),
            border: Border {
                color: mt::OUTLINE_VARIANT,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: mt::with_alpha(Color::BLACK, 0.04),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            },
            text_color: Some(mt::ON_SURFACE),
            ..Default::default()
        })
        .into()
}

pub fn server_tile<'a, Message: 'a + Clone>(
    icon: char,
    name: &'a str,
    address: &'a str,
    auth: bool,
    selected: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let icon_color = if selected { mt::ON_PRIMARY_CONTAINER } else { mt::PRIMARY };
    let name_color = if selected { mt::ON_PRIMARY_CONTAINER } else { mt::ON_SURFACE };

    let (name_display, name_truncated) = truncate(name, 18);
    let (address_display, address_truncated) = truncate(address, 22);
    let any_truncated = name_truncated || address_truncated;

    let base_icon = text(icon).font(crate::icons::FA_SOLID).size(38).color(icon_color);
    let icon_container = container(base_icon)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fixed(44.0))
        .height(Length::Fixed(44.0));
    let icon_widget: Element<'a, Message> = if auth {
        stack![
            icon_container,
            container(
                container(text(crate::icons::KEY).font(crate::icons::FA_SOLID).size(9).color(mt::SURFACE))
                    .padding(2)
                    .style(|_| container::Style {
                        background: Some(Background::Color(mt::WARNING)),
                        border: Border {
                            color: mt::WARNING,
                            width: 1.0,
                            radius: 10.0.into(),
                        },
                        ..Default::default()
                    })
            )
            .align_x(iced::Alignment::End)
            .align_y(iced::Alignment::End)
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0)),
        ]
        .width(Length::Fixed(44.0))
        .height(Length::Fixed(44.0))
        .into()
    } else {
        icon_container.into()
    };

    let content = column![
        icon_widget,
        v_space(10.0),
        text(name_display)
            .size(14)
            .color(name_color)
            .width(Length::Fill)
            .align_x(iced::Alignment::Center)
            .wrapping(Wrapping::None),
        text(address_display)
            .size(11)
            .color(mt::ON_SURFACE_VARIANT)
            .width(Length::Fill)
            .align_x(iced::Alignment::Center)
            .wrapping(Wrapping::None),
    ]
    .spacing(2)
    .width(Length::Fill)
    .align_x(iced::Alignment::Center);

    let inner = container(content)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    let mut btn = button(inner)
        .padding(0)
        .width(Length::Fixed(150.0))
        .height(Length::Fixed(130.0))
        .style(move |_, status| {
            let base_bg = if selected { mt::PRIMARY_CONTAINER } else { mt::SURFACE };
            let bg = match status {
                button::Status::Hovered => {
                    if selected {
                        mt::PRIMARY_CONTAINER
                    } else {
                        mt::with_alpha(mt::PRIMARY, 0.06)
                    }
                }
                button::Status::Pressed => mt::with_alpha(mt::PRIMARY, 0.12),
                _ => base_bg,
            };
            let border_color = if selected { mt::PRIMARY } else { mt::OUTLINE_VARIANT };
            let border_width = if selected { 1.5 } else { 1.0 };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: mt::ON_SURFACE,
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: 12.0.into(),
                },
                shadow: Shadow::default(),
                ..Default::default()
            }
        });
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    if !any_truncated && !auth {
        return btn.into();
    }

    let mut tip_items: Vec<Element<'a, Message>> = Vec::new();
    if name_truncated {
        tip_items.push(text(name.to_string()).size(13).color(mt::ON_SURFACE).into());
    }
    if address_truncated {
        tip_items.push(text(address.to_string()).size(11).color(mt::ON_SURFACE_VARIANT).into());
    }
    if auth {
        tip_items.push(
            row![
                text(crate::icons::LOCK).font(crate::icons::FA_SOLID).size(11).color(mt::WARNING),
                text("Passphrase required").size(11).color(mt::WARNING),
            ]
            .spacing(4)
            .into(),
        );
    }

    let tip = container(column(tip_items).spacing(2))
        .padding(Padding::from([6, 10]))
        .style(|_| container::Style {
            background: Some(Background::Color(mt::SURFACE)),
            border: Border {
                color: mt::OUTLINE_VARIANT,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow {
                color: mt::with_alpha(Color::BLACK, 0.18),
                offset: Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        });
    tooltip(btn, tip, tooltip::Position::Bottom)
        .gap(6.0)
        .into()
}

fn truncate(s: &str, max: usize) -> (String, bool) {
    if s.chars().count() <= max {
        (s.to_string(), false)
    } else {
        let head: String = s.chars().take(max.saturating_sub(3)).collect();
        (format!("{head}..."), true)
    }
}

pub fn icon_pick<'a, Message: 'a + Clone>(
    icon: char,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let icon_color = if selected { mt::ON_PRIMARY_CONTAINER } else { mt::PRIMARY };

    button(
        container(text(icon).font(crate::icons::FA_SOLID).size(24).color(icon_color))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(on_press)
    .padding(0)
    .width(Length::Fixed(56.0))
    .height(Length::Fixed(56.0))
    .style(move |_, status| {
        let base_bg = if selected { mt::PRIMARY_CONTAINER } else { mt::SURFACE };
        let bg = match status {
            button::Status::Hovered => {
                if selected {
                    mt::PRIMARY_CONTAINER
                } else {
                    mt::with_alpha(mt::PRIMARY, 0.06)
                }
            }
            button::Status::Pressed => mt::with_alpha(mt::PRIMARY, 0.12),
            _ => base_bg,
        };
        let border_color = if selected { mt::PRIMARY } else { mt::OUTLINE_VARIANT };
        let border_width = if selected { 1.5 } else { 1.0 };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: mt::ON_SURFACE,
            border: Border {
                color: border_color,
                width: border_width,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    })
    .into()
}

pub fn section_title<'a, Message: 'a>(label: &'a str) -> Element<'a, Message> {
    text(label).size(22).color(mt::ON_SURFACE).into()
}

pub fn field_label<'a, Message: 'a>(label: &'a str) -> Element<'a, Message> {
    text(label).size(13).color(mt::ON_SURFACE_VARIANT).into()
}

pub fn helper_text<'a, Message: 'a>(label: &'a str) -> Element<'a, Message> {
    text(label).size(12).color(mt::ON_SURFACE_VARIANT).into()
}

pub fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    container(Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(mt::OUTLINE_VARIANT)),
            ..Default::default()
        })
        .into()
}

fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

pub fn sidebar<'a, Message: 'a>(
    items: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(items)
        .width(Length::Fixed(232.0))
        .height(Length::Fill)
        .padding(Padding::from([16, 12]))
        .style(|_| container::Style {
            background: Some(Background::Color(mt::SURFACE_CONTAINER)),
            border: Border {
                color: mt::OUTLINE_VARIANT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn corner_handle<'a, Message: 'a + Clone>(on_press: Message) -> Element<'a, Message> {
    fn dot<'a, Message: 'a>() -> Element<'a, Message> {
        container(Space::new())
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(3.0))
            .style(|_| container::Style {
                background: Some(Background::Color(mt::ON_SURFACE_VARIANT)),
                border: Border {
                    radius: 1.5.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    let gap = 4.0;
    let pattern = column![
        row![h_space(gap * 2.0), dot()].spacing(0),
        row![h_space(gap), dot(), h_space(gap - 3.0), dot()].spacing(0),
        row![dot(), h_space(gap - 3.0), dot(), h_space(gap - 3.0), dot()].spacing(0),
    ]
    .spacing(2);

    let visual = container(pattern)
        .padding(Padding::from(4))
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(20.0));

    mouse_area(visual)
        .on_press(on_press)
        .interaction(iced::mouse::Interaction::ResizingDiagonallyDown)
        .into()
}

pub fn about_page<'a, Message: 'a + Clone>(on_url_press: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    let icon = image(ABOUT_ICON.clone());

    let body = card(
        column![
            container(icon).center_x(Length::Fill),
            text("Spud").size(32).font(Font { weight: font::Weight::Bold, ..Font::default() }).color(mt::ON_SURFACE).center().width(Length::Fill),
            v_space(20.0),
            text("A cross-platform remote control application, optimised for gaming.")
                .size(16)
                .color(mt::ON_SURFACE_VARIANT)
                .center()
                .width(Length::Fill),
            v_space(50.0),
            divider(),
            v_space(16.0),
            row![
                text("Version").size(14).color(mt::ON_SURFACE_VARIANT),
                h_space_fill(),
                text(env!("CARGO_PKG_VERSION")).size(14).color(mt::ON_SURFACE),
            ],
            v_space(6.0),
            row![
                text("Contribute").size(14).color(mt::ON_SURFACE_VARIANT),
                h_space_fill(),
                button(text("https://github.com/xfoa/spud").size(14))
                    .on_press(on_url_press("https://github.com/xfoa/spud".to_string()))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: None,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        text_color: match status {
                            button::Status::Hovered | button::Status::Pressed => mt::PRIMARY,
                            _ => darken(mt::PRIMARY, 0.2),
                        },
                        snap: false,
                    }),
            ],
            v_space(6.0),
            row![
                text("License").size(14).color(mt::ON_SURFACE_VARIANT),
                h_space_fill(),
                button(text("GPL-3.0").size(14))
                    .on_press(on_url_press("https://www.gnu.org/licenses/gpl-3.0.en.html".to_string()))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: None,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        text_color: match status {
                            button::Status::Hovered | button::Status::Pressed => mt::PRIMARY,
                            _ => darken(mt::PRIMARY, 0.2),
                        },
                        snap: false,
                    }),
            ],
            v_space(6.0),
            row![
                text("Author").size(14).color(mt::ON_SURFACE_VARIANT),
                h_space_fill(),
                text("foax").size(14).color(mt::ON_SURFACE),
            ],
        ]
        .spacing(0)
        .width(Length::Fill),
    );

    page_body("About", body)
}

pub fn page_body<'a, Message: 'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![section_title(title), v_space(16.0), content.into()]
            .spacing(0)
            .width(Length::Fill),
    )
    .padding(32)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
