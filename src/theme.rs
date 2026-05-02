use iced::{theme::Palette, Color, Theme};

pub const PRIMARY: Color = Color { r: 0.28, g: 0.21, b: 0.14, a: 1.0 };
pub const ON_PRIMARY: Color = Color { r: 0.97, g: 0.95, b: 0.91, a: 1.0 };
pub const PRIMARY_CONTAINER: Color = Color { r: 0.91, g: 0.86, b: 0.78, a: 1.0 };
pub const ON_PRIMARY_CONTAINER: Color = Color { r: 0.13, g: 0.09, b: 0.05, a: 1.0 };

pub const SURFACE: Color = Color { r: 0.98, g: 0.96, b: 0.93, a: 1.0 };
pub const SURFACE_CONTAINER: Color = Color { r: 0.84, g: 0.78, b: 0.70, a: 1.0 };
pub const BACKGROUND: Color = Color { r: 0.96, g: 0.94, b: 0.90, a: 1.0 };

pub const ON_SURFACE: Color = Color { r: 0.13, g: 0.10, b: 0.06, a: 1.0 };
pub const ON_SURFACE_VARIANT: Color = Color { r: 0.30, g: 0.24, b: 0.17, a: 1.0 };
pub const OUTLINE: Color = Color { r: 0.56, g: 0.50, b: 0.42, a: 1.0 };
pub const OUTLINE_VARIANT: Color = Color { r: 0.80, g: 0.74, b: 0.65, a: 1.0 };

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
