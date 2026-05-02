use iced::{theme::Palette, Color, Theme};

pub const PRIMARY: Color = Color { r: 0.88, g: 0.70, b: 0.02, a: 1.0 };
pub const ON_PRIMARY: Color = Color { r: 0.10, g: 0.07, b: 0.00, a: 1.0 };
pub const PRIMARY_CONTAINER: Color = Color { r: 0.98, g: 0.94, b: 0.72, a: 1.0 };
pub const ON_PRIMARY_CONTAINER: Color = Color { r: 0.16, g: 0.11, b: 0.00, a: 1.0 };

pub const SURFACE: Color = Color { r: 1.00, g: 1.00, b: 0.99, a: 1.0 };
pub const SURFACE_CONTAINER: Color = Color { r: 0.99, g: 0.98, b: 0.94, a: 1.0 };
pub const BACKGROUND: Color = Color { r: 1.00, g: 0.99, b: 0.97, a: 1.0 };

pub const ON_SURFACE: Color = Color { r: 0.12, g: 0.10, b: 0.02, a: 1.0 };
pub const ON_SURFACE_VARIANT: Color = Color { r: 0.38, g: 0.32, b: 0.12, a: 1.0 };
pub const OUTLINE: Color = Color { r: 0.72, g: 0.66, b: 0.44, a: 1.0 };
pub const OUTLINE_VARIANT: Color = Color { r: 0.92, g: 0.90, b: 0.82, a: 1.0 };

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
