use iced::{theme::Palette, Color, Theme};

pub const PRIMARY: Color = Color { r: 0.40, g: 0.31, b: 0.64, a: 1.0 };
pub const ON_PRIMARY: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
pub const PRIMARY_CONTAINER: Color = Color { r: 0.91, g: 0.86, b: 0.99, a: 1.0 };
pub const ON_PRIMARY_CONTAINER: Color = Color { r: 0.13, g: 0.05, b: 0.36, a: 1.0 };

pub const SURFACE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
pub const SURFACE_CONTAINER: Color = Color { r: 0.96, g: 0.95, b: 0.98, a: 1.0 };
pub const BACKGROUND: Color = Color { r: 0.985, g: 0.98, b: 0.99, a: 1.0 };

pub const ON_SURFACE: Color = Color { r: 0.11, g: 0.11, b: 0.13, a: 1.0 };
pub const ON_SURFACE_VARIANT: Color = Color { r: 0.30, g: 0.30, b: 0.34, a: 1.0 };
pub const OUTLINE: Color = Color { r: 0.78, g: 0.78, b: 0.81, a: 1.0 };
pub const OUTLINE_VARIANT: Color = Color { r: 0.88, g: 0.87, b: 0.90, a: 1.0 };

pub const SUCCESS: Color = Color { r: 0.30, g: 0.69, b: 0.31, a: 1.0 };
pub const WARNING: Color = Color { r: 0.96, g: 0.62, b: 0.04, a: 1.0 };
pub const DANGER: Color = Color { r: 0.72, g: 0.21, b: 0.21, a: 1.0 };

pub fn material_theme() -> Theme {
    let palette = Palette {
        background: BACKGROUND,
        text: ON_SURFACE,
        primary: PRIMARY,
        success: SUCCESS,
        warning: WARNING,
        danger: DANGER,
    };
    Theme::custom("Material".to_string(), palette)
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
