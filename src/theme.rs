use iced::{theme::Palette, Color, Theme};

pub const PRIMARY: Color = Color { r: 0.92, g: 0.76, b: 0.10, a: 1.0 };
pub const ON_PRIMARY: Color = Color { r: 0.10, g: 0.07, b: 0.00, a: 1.0 };
pub const PRIMARY_CONTAINER: Color = Color { r: 0.98, g: 0.94, b: 0.72, a: 1.0 };
pub const ON_PRIMARY_CONTAINER: Color = Color { r: 0.16, g: 0.11, b: 0.00, a: 1.0 };

pub const SURFACE: Color = Color { r: 1.00, g: 1.00, b: 0.99, a: 1.0 };
pub const SURFACE_CONTAINER: Color = Color { r: 0.99, g: 0.98, b: 0.94, a: 1.0 };
pub const BACKGROUND: Color = Color { r: 1.00, g: 0.99, b: 0.97, a: 1.0 };

pub const ON_SURFACE: Color = Color { r: 0.11, g: 0.10, b: 0.06, a: 1.0 };
pub const ON_SURFACE_VARIANT: Color = Color { r: 0.36, g: 0.32, b: 0.22, a: 1.0 };
pub const OUTLINE: Color = Color { r: 0.66, g: 0.62, b: 0.52, a: 1.0 };
pub const OUTLINE_VARIANT: Color = Color { r: 0.91, g: 0.89, b: 0.84, a: 1.0 };

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
