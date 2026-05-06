use iced::window::settings::PlatformSpecific;

mod app;
mod components;
mod cert;
mod config;
mod crypto;
mod discovery;
mod icons;
mod input;
mod net;
mod session;
mod theme;
mod views;

fn main() -> iced::Result {
    let icon = iced::window::icon::from_file_data(
        include_bytes!("../resources/icon.png"),
        None,
    )
    .ok();

    let app_name = "Spud";
    iced::application(app::Spud::default, app::Spud::update, app::Spud::view)
        .title(app::Spud::title)
        .theme(app::Spud::theme)
        .subscription(app::Spud::subscription)
        .font(icons::FA_SOLID_BYTES)
        .window_size(iced::Size::new(1000.0, 650.0))
        .window(iced::window::Settings {
            icon,
            min_size: Some(iced::Size::new(800.0, 600.0)),
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: app_name.to_string(),
                ..PlatformSpecific::default()
            },
            ..Default::default()
        })
        .run()
}
