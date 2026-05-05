use iced::font::{Family, Weight};
use iced::Font;

pub const FA_SOLID_BYTES: &[u8] = include_bytes!("../resources/fa-solid-900.otf");

pub const FA_SOLID: Font = Font {
    family: Family::Name("Font Awesome 7 Free"),
    weight: Weight::Black,
    ..Font::DEFAULT
};

pub const PLUG: char = '\u{f1e6}';
pub const COMPUTER_MOUSE: char = '\u{f8cc}';
pub const KEYBOARD: char = '\u{f11c}';
pub const CIRCLE_INFO: char = '\u{f05a}';
pub const SIGNAL: char = '\u{f012}';
pub const NETWORK_WIRED: char = '\u{f6ff}';
pub const SHIELD_HALVED: char = '\u{f3ed}';
pub const LOCK: char = '\u{f023}';
pub const TRIANGLE_EXCLAMATION: char = '\u{f071}';
pub const DESKTOP: char = '\u{f108}';
pub const LAPTOP: char = '\u{f109}';
pub const SERVER: char = '\u{f233}';
pub const GEAR: char = '\u{f013}';
