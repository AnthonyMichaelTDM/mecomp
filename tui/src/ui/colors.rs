use material::{colors, HexColor};

pub struct Color(pub HexColor);

impl From<Color> for ratatui::style::Color {
    /// Converts to a Ratatui Color from the `HexColor`.
    fn from(value: Color) -> Self {
        let s = value.0.to_string();
        let (r, g, b) = (
            u8::from_str_radix(&s[1..3], 16).unwrap_or_default(),
            u8::from_str_radix(&s[3..5], 16).unwrap_or_default(),
            u8::from_str_radix(&s[5..7], 16).unwrap_or_default(),
        );

        Self::Rgb(r, g, b)
    }
}

// app border colors
pub const APP_BORDER: Color = Color(colors::PINK_900);
pub const APP_BORDER_TEXT: Color = Color(colors::PINK_300);

// border colors
pub const BORDER_UNFOCUSED: Color = Color(colors::RED_900);
pub const BORDER_FOCUSED: Color = Color(colors::RED_200);

// Popup border colors
pub const POPUP_BORDER: Color = Color(colors::LIGHT_BLUE_500);

// text colors
pub const TEXT_NORMAL: Color = Color(colors::WHITE);
pub const TEXT_HIGHLIGHT: Color = Color(colors::RED_600);
pub const TEXT_HIGHLIGHT_ALT: Color = Color(colors::RED_200);

// gauge colors, such as song progress bar
pub const GAUGE_FOREGROUND: Color = Color(colors::WHITE);
pub const GAUGE_BACKGROUND: Color = Color(colors::BLACK);
