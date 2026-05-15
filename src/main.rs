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
    let _args: Vec<String> = std::env::args().collect();
    #[cfg(target_os = "linux")]
    if _args.len() >= 2 && _args[1] == "injection-helper" {
        let socket_path = _args.get(2).cloned().unwrap_or_else(|| "/tmp/spud-input.sock".to_string());
        let screen_width = _args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1920);
        let screen_height = _args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1080);
        if let Err(e) = input::helper::run(&socket_path, screen_width, screen_height) {
            eprintln!("[spud-injection-helper] failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .contains("COSMIC")
        && std::env::var("WAYLAND_DISPLAY").is_ok()
    {
        eprintln!("[spud] COSMIC detected.");
        eprintln!("[spud] If fullscreen capture doesn't work, try: WAYLAND_DISPLAY= ./spud");
        eprintln!("[spud] (e.g. VM tablet input can interfere with pointer grab)");
    }

    let icon = iced::window::icon::from_file_data(
        include_bytes!("../resources/icon.png"),
        None,
    )
    .ok();

    let _app_name = "Spud";
    iced::application(app::Spud::default, app::Spud::update, app::Spud::view)
        .title(app::Spud::title)
        .theme(app::Spud::theme)
        .subscription(app::Spud::subscription)
        .font(icons::FA_SOLID_BYTES)
        .window_size(iced::Size::new(1000.0, 650.0))
        .window(iced::window::Settings {
            icon,
            min_size: Some(iced::Size::new(800.0, 600.0)),
            platform_specific: PlatformSpecific::default(),
            ..Default::default()
        })
        .run()
}
