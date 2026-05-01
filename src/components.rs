use iced::widget::{button, column, container, mouse_area, row, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme as mt;

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
    let label_color = if active { mt::PRIMARY } else { mt::ON_SURFACE_VARIANT };

    let indicator_color = if active { mt::PRIMARY } else { Color::TRANSPARENT };

    let content = column![
        container(text(label).size(15).color(label_color))
            .center_x(Length::Fill)
            .padding(Padding::from([14, 24])),
        container(Space::new().height(Length::Fixed(3.0)))
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
            let hover_overlay = matches!(
                status,
                button::Status::Hovered | button::Status::Pressed
            );
            let bg = if hover_overlay {
                Some(Background::Color(mt::with_alpha(mt::PRIMARY, 0.06)))
            } else {
                Some(Background::Color(Color::TRANSPARENT))
            };
            button::Style {
                background: bg,
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
    icon_name: &'a str,
    active: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let (text_color, icon_color) = if active {
        (mt::ON_PRIMARY_CONTAINER, mt::ON_PRIMARY_CONTAINER)
    } else {
        (mt::ON_SURFACE, mt::ON_SURFACE_VARIANT)
    };

    let icon = container(
        iced_font_awesome::fa_icon_solid(icon_name)
            .size(16.0)
            .color(icon_color),
    )
    .center_x(Length::Fixed(22.0));

    let content = row![icon, text(label).size(14).color(text_color)]
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
    on_press: Message,
) -> Element<'a, Message> {
    button(
        container(text(label).size(14).color(mt::ON_PRIMARY))
            .padding(Padding::from([10, 24])),
    )
    .on_press(on_press)
    .padding(0)
    .style(|_, status| {
        let bg = match status {
            button::Status::Hovered => darken(mt::PRIMARY, 0.05),
            button::Status::Pressed => darken(mt::PRIMARY, 0.10),
            _ => mt::PRIMARY,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: mt::ON_PRIMARY,
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
            shadow: Shadow {
                color: mt::with_alpha(Color::BLACK, 0.15),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            },
            ..Default::default()
        }
    })
    .into()
}

pub fn outlined_button<'a, Message: 'a + Clone>(
    label: &'a str,
    on_press: Message,
) -> Element<'a, Message> {
    button(
        container(text(label).size(14).color(mt::PRIMARY))
            .padding(Padding::from([10, 24])),
    )
    .on_press(on_press)
    .padding(0)
    .style(|_, status| {
        let bg = match status {
            button::Status::Hovered => mt::with_alpha(mt::PRIMARY, 0.06),
            button::Status::Pressed => mt::with_alpha(mt::PRIMARY, 0.12),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: mt::PRIMARY,
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

pub fn about_page<'a, Message: 'a>() -> Element<'a, Message> {
    let body = card(
        column![
            text("Spud").size(20).color(mt::ON_SURFACE),
            v_space(6.0),
            helper_text("A cross-platform remote control application."),
            v_space(16.0),
            divider(),
            v_space(16.0),
            row![
                text("Version").size(14).color(mt::ON_SURFACE_VARIANT),
                h_space_fill(),
                text("0.1.0").size(14).color(mt::ON_SURFACE),
            ],
        ]
        .spacing(0),
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
