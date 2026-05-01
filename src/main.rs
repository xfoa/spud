mod app;
mod components;
mod theme;
mod views;

fn main() -> iced::Result {
    iced::application(app::Spud::default, app::Spud::update, app::Spud::view)
        .title("Spud")
        .theme(app::Spud::theme)
        .window_size(iced::Size::new(1000.0, 650.0))
        .window(iced::window::Settings {
            min_size: Some(iced::Size::new(640.0, 480.0)),
            ..Default::default()
        })
        .run()
}
