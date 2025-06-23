use std::sync::Once;

use mecomp_core::OnceLockDefault;

// app border colors
pub static APP_BORDER: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::PINK_900);
pub static APP_BORDER_TEXT: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::PINK_300);

// border colors
pub static BORDER_UNFOCUSED: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::RED_900);
pub static BORDER_FOCUSED: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::RED_200);

// Popup border colors
pub static POPUP_BORDER: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::LIGHT_BLUE_500);

// text colors
pub static TEXT_NORMAL: OnceLockDefault<material::HexColor> = OnceLockDefault::new(material::WHITE);
pub static TEXT_HIGHLIGHT: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::RED_600);
pub static TEXT_HIGHLIGHT_ALT: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::RED_200);

// gauge colors, such as song progress bar
pub static GAUGE_FILLED: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::WHITE);
pub static GAUGE_UNFILLED: OnceLockDefault<material::HexColor> =
    OnceLockDefault::new(material::BLACK);

pub static COLORS_INITIALIZED: Once = Once::new();

/// Initialize the colors for the app at once.
///
/// # Memory
///
/// This function uses `Box::leak` to leak the memory of the string slices.
///
/// This shouldn't be a problem though, since this function is only called once as part of the app initialization.
pub fn initialize_colors(theme: mecomp_core::config::TuiColorScheme) {
    macro_rules! set_color {
        ($color:ident, $value:expr) => {
            if let Some(color) =
                $value.and_then(|c| material::HexColor::parse(Box::leak(c.into_boxed_str())))
            {
                $color.set(color).ok();
            }
        };
    }

    // Initialize the colors only once
    COLORS_INITIALIZED.call_once(|| {
        // Set the colors
        set_color!(APP_BORDER, theme.app_border);
        set_color!(APP_BORDER_TEXT, theme.app_border_text);
        set_color!(BORDER_UNFOCUSED, theme.border_unfocused);
        set_color!(BORDER_FOCUSED, theme.border_focused);
        set_color!(POPUP_BORDER, theme.popup_border);
        set_color!(TEXT_NORMAL, theme.text_normal);
        set_color!(TEXT_HIGHLIGHT, theme.text_highlight);
        set_color!(TEXT_HIGHLIGHT_ALT, theme.text_highlight_alt);
        set_color!(GAUGE_FILLED, theme.gauge_filled);
        set_color!(GAUGE_UNFILLED, theme.gauge_unfilled);
    });
}

#[must_use]
pub fn border_color(is_focused: bool) -> material::HexColor {
    if is_focused {
        *BORDER_FOCUSED
    } else {
        *BORDER_UNFOCUSED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mecomp_core::config::TuiColorScheme;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_border_color() {
        let focused = border_color(true);
        let unfocused = border_color(false);
        assert_eq!(focused, *BORDER_FOCUSED);
        assert_eq!(unfocused, *BORDER_UNFOCUSED);
    }

    #[test]
    fn test_colors() {
        // Test that the default colors are set correctly
        assert_eq!(*APP_BORDER, material::PINK_900);
        assert_eq!(*APP_BORDER_TEXT, material::PINK_300);
        assert_eq!(*BORDER_UNFOCUSED, material::RED_900);
        assert_eq!(*BORDER_FOCUSED, material::RED_200);
        assert_eq!(*POPUP_BORDER, material::LIGHT_BLUE_500);
        assert_eq!(*TEXT_NORMAL, material::WHITE);
        assert_eq!(*TEXT_HIGHLIGHT, material::RED_600);
        assert_eq!(*TEXT_HIGHLIGHT_ALT, material::RED_200);
        assert_eq!(*GAUGE_FILLED, material::WHITE);
        assert_eq!(*GAUGE_UNFILLED, material::BLACK);

        // Test that the colors are initialized only once
        let theme = TuiColorScheme {
            app_border: Some(material::PINK_900.to_string()),
            app_border_text: Some(material::PINK_300.to_string()),
            border_unfocused: Some(material::RED_900.to_string()),
            border_focused: Some(material::RED_200.to_string()),
            popup_border: Some(material::LIGHT_BLUE_500.to_string()),
            text_normal: Some(material::WHITE.to_string()),
            text_highlight: Some(material::RED_600.to_string()),
            text_highlight_alt: Some(material::RED_200.to_string()),
            gauge_filled: Some(material::WHITE.to_string()),
            gauge_unfilled: Some(material::BLACK.to_string()),
        };

        // Initialize the colors
        initialize_colors(theme);

        // Test that the colors are set
        assert!(APP_BORDER.is_initialized());
        assert!(APP_BORDER_TEXT.is_initialized());
        assert!(BORDER_UNFOCUSED.is_initialized());
        assert!(BORDER_FOCUSED.is_initialized());
        assert!(POPUP_BORDER.is_initialized());
        assert!(TEXT_NORMAL.is_initialized());
        assert!(TEXT_HIGHLIGHT.is_initialized());
        assert!(TEXT_HIGHLIGHT_ALT.is_initialized());
        assert!(GAUGE_FILLED.is_initialized());
        assert!(GAUGE_UNFILLED.is_initialized());
    }
}

pub mod material {
    //! # Material Design Colors
    //!
    //! This module provides the 2014 Material Design System [color palettes] as constants.
    //!
    //! [color palettes]: https://m2.material.io/design/color/the-color-system.html#tools-for-picking-colors
    //!
    //! Ripped from [the material crate](https://github.com/azorng/material/blob/0b6205cf2e6750d92d0ecf7de5779bbf5caa1838/src/lib.rs#L64-L598),
    //! which we don't use directly because it brings in other things we don't need.
    #![allow(dead_code)]

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HexColor(&'static str);

    impl HexColor {
        /// Parse a hex color string.
        /// Can be a hex string like `#ff0000` or a material color name like `red_500` or `LIGHT_BLUE_100`.
        /// Returns a `HexColor` if the string is valid, otherwise returns None.
        #[must_use]
        pub fn parse(s: &'static str) -> Option<Self> {
            // First parse the string into a hex color
            let candidate = if s.len() == 7 && s.starts_with('#') {
                Self(s)
            } else if let Some((_, hex)) = MATERIAL_COLORS
                .iter()
                .find(|(name, _)| *name == s.to_ascii_lowercase())
            {
                *hex
            } else {
                return None;
            };

            // double check that the hex color is valid
            if candidate.0.len() == 7
                && candidate.0.starts_with('#')
                && candidate.0[1..].chars().all(|c| c.is_ascii_hexdigit())
            {
                Some(candidate)
            } else {
                None
            }
        }
    }

    impl From<HexColor> for ratatui::style::Color {
        /// Converts to a Ratatui Color from the `HexColor`.
        fn from(hex_color: HexColor) -> Self {
            let s = hex_color.0;
            let (r, g, b) = (
                u8::from_str_radix(&s[1..3], 16).unwrap_or_default(),
                u8::from_str_radix(&s[3..5], 16).unwrap_or_default(),
                u8::from_str_radix(&s[5..7], 16).unwrap_or_default(),
            );

            Self::Rgb(r, g, b)
        }
    }

    impl std::fmt::Display for HexColor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    /// a lookup table for the colors
    static MATERIAL_COLORS: &[(&str, HexColor)] = &[
        // reds
        ("red_50", RED_50),
        ("red_100", RED_100),
        ("red_200", RED_200),
        ("red_300", RED_300),
        ("red_400", RED_400),
        ("red_500", RED_500),
        ("red_600", RED_600),
        ("red_700", RED_700),
        ("red_800", RED_800),
        ("red_900", RED_900),
        ("red_a100", RED_A100),
        ("red_a200", RED_A200),
        ("red_a400", RED_A400),
        ("red_a700", RED_A700),
        // pinks
        ("pink_50", PINK_50),
        ("pink_100", PINK_100),
        ("pink_200", PINK_200),
        ("pink_300", PINK_300),
        ("pink_400", PINK_400),
        ("pink_500", PINK_500),
        ("pink_600", PINK_600),
        ("pink_700", PINK_700),
        ("pink_800", PINK_800),
        ("pink_900", PINK_900),
        ("pink_a100", PINK_A100),
        ("pink_a200", PINK_A200),
        ("pink_a400", PINK_A400),
        ("pink_a700", PINK_A700),
        // purples
        ("purple_50", PURPLE_50),
        ("purple_100", PURPLE_100),
        ("purple_200", PURPLE_200),
        ("purple_300", PURPLE_300),
        ("purple_400", PURPLE_400),
        ("purple_500", PURPLE_500),
        ("purple_600", PURPLE_600),
        ("purple_700", PURPLE_700),
        ("purple_800", PURPLE_800),
        ("purple_900", PURPLE_900),
        ("purple_a100", PURPLE_A100),
        ("purple_a200", PURPLE_A200),
        ("purple_a400", PURPLE_A400),
        ("purple_a700", PURPLE_A700),
        // deep purples
        ("deep_purple_50", DEEP_PURPLE_50),
        ("deep_purple_100", DEEP_PURPLE_100),
        ("deep_purple_200", DEEP_PURPLE_200),
        ("deep_purple_300", DEEP_PURPLE_300),
        ("deep_purple_400", DEEP_PURPLE_400),
        ("deep_purple_500", DEEP_PURPLE_500),
        ("deep_purple_600", DEEP_PURPLE_600),
        ("deep_purple_700", DEEP_PURPLE_700),
        ("deep_purple_800", DEEP_PURPLE_800),
        ("deep_purple_900", DEEP_PURPLE_900),
        ("deep_purple_a100", DEEP_PURPLE_A100),
        ("deep_purple_a200", DEEP_PURPLE_A200),
        ("deep_purple_a400", DEEP_PURPLE_A400),
        ("deep_purple_a700", DEEP_PURPLE_A700),
        // indigos
        ("indigo_50", INDIGO_50),
        ("indigo_100", INDIGO_100),
        ("indigo_200", INDIGO_200),
        ("indigo_300", INDIGO_300),
        ("indigo_400", INDIGO_400),
        ("indigo_500", INDIGO_500),
        ("indigo_600", INDIGO_600),
        ("indigo_700", INDIGO_700),
        ("indigo_800", INDIGO_800),
        ("indigo_900", INDIGO_900),
        ("indigo_a100", INDIGO_A100),
        ("indigo_a200", INDIGO_A200),
        ("indigo_a400", INDIGO_A400),
        ("indigo_a700", INDIGO_A700),
        // blues
        ("blue_50", BLUE_50),
        ("blue_100", BLUE_100),
        ("blue_200", BLUE_200),
        ("blue_300", BLUE_300),
        ("blue_400", BLUE_400),
        ("blue_500", BLUE_500),
        ("blue_600", BLUE_600),
        ("blue_700", BLUE_700),
        ("blue_800", BLUE_800),
        ("blue_900", BLUE_900),
        ("blue_a100", BLUE_A100),
        ("blue_a200", BLUE_A200),
        ("blue_a400", BLUE_A400),
        ("blue_a700", BLUE_A700),
        // light blues
        ("light_blue_50", LIGHT_BLUE_50),
        ("light_blue_100", LIGHT_BLUE_100),
        ("light_blue_200", LIGHT_BLUE_200),
        ("light_blue_300", LIGHT_BLUE_300),
        ("light_blue_400", LIGHT_BLUE_400),
        ("light_blue_500", LIGHT_BLUE_500),
        ("light_blue_600", LIGHT_BLUE_600),
        ("light_blue_700", LIGHT_BLUE_700),
        ("light_blue_800", LIGHT_BLUE_800),
        ("light_blue_900", LIGHT_BLUE_900),
        ("light_blue_a100", LIGHT_BLUE_A100),
        ("light_blue_a200", LIGHT_BLUE_A200),
        ("light_blue_a400", LIGHT_BLUE_A400),
        ("light_blue_a700", LIGHT_BLUE_A700),
        // cyans
        ("cyan_50", CYAN_50),
        ("cyan_100", CYAN_100),
        ("cyan_200", CYAN_200),
        ("cyan_300", CYAN_300),
        ("cyan_400", CYAN_400),
        ("cyan_500", CYAN_500),
        ("cyan_600", CYAN_600),
        ("cyan_700", CYAN_700),
        ("cyan_800", CYAN_800),
        ("cyan_900", CYAN_900),
        ("cyan_a100", CYAN_A100),
        ("cyan_a200", CYAN_A200),
        ("cyan_a400", CYAN_A400),
        ("cyan_a700", CYAN_A700),
        // teals
        ("teal_50", TEAL_50),
        ("teal_100", TEAL_100),
        ("teal_200", TEAL_200),
        ("teal_300", TEAL_300),
        ("teal_400", TEAL_400),
        ("teal_500", TEAL_500),
        ("teal_600", TEAL_600),
        ("teal_700", TEAL_700),
        ("teal_800", TEAL_800),
        ("teal_900", TEAL_900),
        ("teal_a100", TEAL_A100),
        ("teal_a200", TEAL_A200),
        ("teal_a400", TEAL_A400),
        ("teal_a700", TEAL_A700),
        // greens
        ("green_50", GREEN_50),
        ("green_100", GREEN_100),
        ("green_200", GREEN_200),
        ("green_300", GREEN_300),
        ("green_400", GREEN_400),
        ("green_500", GREEN_500),
        ("green_600", GREEN_600),
        ("green_700", GREEN_700),
        ("green_800", GREEN_800),
        ("green_900", GREEN_900),
        ("green_a100", GREEN_A100),
        ("green_a200", GREEN_A200),
        ("green_a400", GREEN_A400),
        ("green_a700", GREEN_A700),
        // light greens
        ("light_green_50", LIGHT_GREEN_50),
        ("light_green_100", LIGHT_GREEN_100),
        ("light_green_200", LIGHT_GREEN_200),
        ("light_green_300", LIGHT_GREEN_300),
        ("light_green_400", LIGHT_GREEN_400),
        ("light_green_500", LIGHT_GREEN_500),
        ("light_green_600", LIGHT_GREEN_600),
        ("light_green_700", LIGHT_GREEN_700),
        ("light_green_800", LIGHT_GREEN_800),
        ("light_green_900", LIGHT_GREEN_900),
        ("light_green_a100", LIGHT_GREEN_A100),
        ("light_green_a200", LIGHT_GREEN_A200),
        ("light_green_a400", LIGHT_GREEN_A400),
        ("light_green_a700", LIGHT_GREEN_A700),
        // limes
        ("lime_50", LIME_50),
        ("lime_100", LIME_100),
        ("lime_200", LIME_200),
        ("lime_300", LIME_300),
        ("lime_400", LIME_400),
        ("lime_500", LIME_500),
        ("lime_600", LIME_600),
        ("lime_700", LIME_700),
        ("lime_800", LIME_800),
        ("lime_900", LIME_900),
        ("lime_a100", LIME_A100),
        ("lime_a200", LIME_A200),
        ("lime_a400", LIME_A400),
        ("lime_a700", LIME_A700),
        // yellows
        ("yellow_50", YELLOW_50),
        ("yellow_100", YELLOW_100),
        ("yellow_200", YELLOW_200),
        ("yellow_300", YELLOW_300),
        ("yellow_400", YELLOW_400),
        ("yellow_500", YELLOW_500),
        ("yellow_600", YELLOW_600),
        ("yellow_700", YELLOW_700),
        ("yellow_800", YELLOW_800),
        ("yellow_900", YELLOW_900),
        ("yellow_a100", YELLOW_A100),
        ("yellow_a200", YELLOW_A200),
        ("yellow_a400", YELLOW_A400),
        ("yellow_a700", YELLOW_A700),
        // amber
        ("amber_50", AMBER_50),
        ("amber_100", AMBER_100),
        ("amber_200", AMBER_200),
        ("amber_300", AMBER_300),
        ("amber_400", AMBER_400),
        ("amber_500", AMBER_500),
        ("amber_600", AMBER_600),
        ("amber_700", AMBER_700),
        ("amber_800", AMBER_800),
        ("amber_900", AMBER_900),
        ("amber_a100", AMBER_A100),
        ("amber_a200", AMBER_A200),
        ("amber_a400", AMBER_A400),
        ("amber_a700", AMBER_A700),
        // oranges
        ("orange_50", ORANGE_50),
        ("orange_100", ORANGE_100),
        ("orange_200", ORANGE_200),
        ("orange_300", ORANGE_300),
        ("orange_400", ORANGE_400),
        ("orange_500", ORANGE_500),
        ("orange_600", ORANGE_600),
        ("orange_700", ORANGE_700),
        ("orange_800", ORANGE_800),
        ("orange_900", ORANGE_900),
        ("orange_a100", ORANGE_A100),
        ("orange_a200", ORANGE_A200),
        ("orange_a400", ORANGE_A400),
        ("orange_a700", ORANGE_A700),
        // deep oranges
        ("deep_orange_50", DEEP_ORANGE_50),
        ("deep_orange_100", DEEP_ORANGE_100),
        ("deep_orange_200", DEEP_ORANGE_200),
        ("deep_orange_300", DEEP_ORANGE_300),
        ("deep_orange_400", DEEP_ORANGE_400),
        ("deep_orange_500", DEEP_ORANGE_500),
        ("deep_orange_600", DEEP_ORANGE_600),
        ("deep_orange_700", DEEP_ORANGE_700),
        ("deep_orange_800", DEEP_ORANGE_800),
        ("deep_orange_900", DEEP_ORANGE_900),
        ("deep_orange_a100", DEEP_ORANGE_A100),
        ("deep_orange_a200", DEEP_ORANGE_A200),
        ("deep_orange_a400", DEEP_ORANGE_A400),
        ("deep_orange_a700", DEEP_ORANGE_A700),
        // browns
        ("brown_50", BROWN_50),
        ("brown_100", BROWN_100),
        ("brown_200", BROWN_200),
        ("brown_300", BROWN_300),
        ("brown_400", BROWN_400),
        ("brown_500", BROWN_500),
        ("brown_600", BROWN_600),
        ("brown_700", BROWN_700),
        ("brown_800", BROWN_800),
        ("brown_900", BROWN_900),
        // grey
        ("grey_50", GREY_50),
        ("grey_100", GREY_100),
        ("grey_200", GREY_200),
        ("grey_300", GREY_300),
        ("grey_400", GREY_400),
        ("grey_500", GREY_500),
        ("grey_600", GREY_600),
        ("grey_700", GREY_700),
        ("grey_800", GREY_800),
        ("grey_900", GREY_900),
        // blue greys
        ("blue_grey_50", BLUE_GREY_50),
        ("blue_grey_100", BLUE_GREY_100),
        ("blue_grey_200", BLUE_GREY_200),
        ("blue_grey_300", BLUE_GREY_300),
        ("blue_grey_400", BLUE_GREY_400),
        ("blue_grey_500", BLUE_GREY_500),
        ("blue_grey_600", BLUE_GREY_600),
        ("blue_grey_700", BLUE_GREY_700),
        ("blue_grey_800", BLUE_GREY_800),
        ("blue_grey_900", BLUE_GREY_900),
        // white and black
        ("white", WHITE),
        ("black", BLACK),
    ];

    /// <span style="color:#ffebee">&#9632;</span> (#ffebee)
    pub const RED_50: HexColor = HexColor("#ffebee");
    /// <span style="color:#ffcdd2">&#9632;</span> (#ffcdd2)
    pub const RED_100: HexColor = HexColor("#ffcdd2");
    /// <span style="color:#ef9a9a">&#9632;</span> (#ef9a9a)
    pub const RED_200: HexColor = HexColor("#ef9a9a");
    /// <span style="color:#e57373">&#9632;</span> (#e57373)
    pub const RED_300: HexColor = HexColor("#e57373");
    /// <span style="color:#ef5350">&#9632;</span> (#ef5350)
    pub const RED_400: HexColor = HexColor("#ef5350");
    /// <span style="color:#f44336">&#9632;</span> (#f44336)
    pub const RED_500: HexColor = HexColor("#f44336");
    /// <span style="color:#e53935">&#9632;</span> (#e53935)
    pub const RED_600: HexColor = HexColor("#e53935");
    /// <span style="color:#d32f2f">&#9632;</span> (#d32f2f)
    pub const RED_700: HexColor = HexColor("#d32f2f");
    /// <span style="color:#c62828">&#9632;</span> (#c62828)
    pub const RED_800: HexColor = HexColor("#c62828");
    /// <span style="color:#b71c1c">&#9632;</span> (#b71c1c)
    pub const RED_900: HexColor = HexColor("#b71c1c");
    /// <span style="color:#ff8a80">&#9632;</span> (#ff8a80)
    pub const RED_A100: HexColor = HexColor("#ff8a80");
    /// <span style="color:#ff5252">&#9632;</span> (#ff5252)
    pub const RED_A200: HexColor = HexColor("#ff5252");
    /// <span style="color:#ff1744">&#9632;</span> (#ff1744)
    pub const RED_A400: HexColor = HexColor("#ff1744");
    /// <span style="color:#d50000">&#9632;</span> (#d50000)
    pub const RED_A700: HexColor = HexColor("#d50000");

    /// <span style="color:#fce4ec">&#9632;</span> (#fce4ec)
    pub const PINK_50: HexColor = HexColor("#fce4ec");
    /// <span style="color:#f8bbd0">&#9632;</span> (#f8bbd0)
    pub const PINK_100: HexColor = HexColor("#f8bbd0");
    /// <span style="color:#f48fb1">&#9632;</span> (#f48fb1)
    pub const PINK_200: HexColor = HexColor("#f48fb1");
    /// <span style="color:#f06292">&#9632;</span> (#f06292)
    pub const PINK_300: HexColor = HexColor("#f06292");
    /// <span style="color:#ec407a">&#9632;</span> (#ec407a)
    pub const PINK_400: HexColor = HexColor("#ec407a");
    /// <span style="color:#e91e63">&#9632;</span> (#e91e63)
    pub const PINK_500: HexColor = HexColor("#e91e63");
    /// <span style="color:#d81b60">&#9632;</span> (#d81b60)
    pub const PINK_600: HexColor = HexColor("#d81b60");
    /// <span style="color:#c2185b">&#9632;</span> (#c2185b)
    pub const PINK_700: HexColor = HexColor("#c2185b");
    /// <span style="color:#ad1457">&#9632;</span> (#ad1457)
    pub const PINK_800: HexColor = HexColor("#ad1457");
    /// <span style="color:#880e4f">&#9632;</span> (#880e4f)
    pub const PINK_900: HexColor = HexColor("#880e4f");
    /// <span style="color:#ff80ab">&#9632;</span> (#ff80ab)
    pub const PINK_A100: HexColor = HexColor("#ff80ab");
    /// <span style="color:#ff4081">&#9632;</span> (#ff4081)
    pub const PINK_A200: HexColor = HexColor("#ff4081");
    /// <span style="color:#f50057">&#9632;</span> (#f50057)
    pub const PINK_A400: HexColor = HexColor("#f50057");
    /// <span style="color:#c51162">&#9632;</span> (#c51162)
    pub const PINK_A700: HexColor = HexColor("#c51162");

    /// <span style="color:#f3e5f5">&#9632;</span> (#f3e5f5)
    pub const PURPLE_50: HexColor = HexColor("#f3e5f5");
    /// <span style="color:#e1bee7">&#9632;</span> (#e1bee7)
    pub const PURPLE_100: HexColor = HexColor("#e1bee7");
    /// <span style="color:#ce93d8">&#9632;</span> (#ce93d8)
    pub const PURPLE_200: HexColor = HexColor("#ce93d8");
    /// <span style="color:#ba68c8">&#9632;</span> (#ba68c8)
    pub const PURPLE_300: HexColor = HexColor("#ba68c8");
    /// <span style="color:#ab47bc">&#9632;</span> (#ab47bc)
    pub const PURPLE_400: HexColor = HexColor("#ab47bc");
    /// <span style="color:#9c27b0">&#9632;</span> (#9c27b0)
    pub const PURPLE_500: HexColor = HexColor("#9c27b0");
    /// <span style="color:#8e24aa">&#9632;</span> (#8e24aa)
    pub const PURPLE_600: HexColor = HexColor("#8e24aa");
    /// <span style="color:#7b1fa2">&#9632;</span> (#7b1fa2)
    pub const PURPLE_700: HexColor = HexColor("#7b1fa2");
    /// <span style="color:#6a1b9a">&#9632;</span> (#6a1b9a)
    pub const PURPLE_800: HexColor = HexColor("#6a1b9a");
    /// <span style="color:#4a148c">&#9632;</span> (#4a148c)
    pub const PURPLE_900: HexColor = HexColor("#4a148c");
    /// <span style="color:#ea80fc">&#9632;</span> (#ea80fc)
    pub const PURPLE_A100: HexColor = HexColor("#ea80fc");
    /// <span style="color:#e040fb">&#9632;</span> (#e040fb)
    pub const PURPLE_A200: HexColor = HexColor("#e040fb");
    /// <span style="color:#d500f9">&#9632;</span> (#d500f9)
    pub const PURPLE_A400: HexColor = HexColor("#d500f9");
    /// <span style="color:#aa00ff">&#9632;</span> (#aa00ff)
    pub const PURPLE_A700: HexColor = HexColor("#aa00ff");

    /// <span style="color:#ede7f6">&#9632;</span> (#ede7f6)
    pub const DEEP_PURPLE_50: HexColor = HexColor("#ede7f6");
    /// <span style="color:#d1c4e9">&#9632;</span> (#d1c4e9)
    pub const DEEP_PURPLE_100: HexColor = HexColor("#d1c4e9");
    /// <span style="color:#b39ddb">&#9632;</span> (#b39ddb)
    pub const DEEP_PURPLE_200: HexColor = HexColor("#b39ddb");
    /// <span style="color:#9575cd">&#9632;</span> (#9575cd)
    pub const DEEP_PURPLE_300: HexColor = HexColor("#9575cd");
    /// <span style="color:#7e57c2">&#9632;</span> (#7e57c2)
    pub const DEEP_PURPLE_400: HexColor = HexColor("#7e57c2");
    /// <span style="color:#673ab7">&#9632;</span> (#673ab7)
    pub const DEEP_PURPLE_500: HexColor = HexColor("#673ab7");
    /// <span style="color:#5e35b1">&#9632;</span> (#5e35b1)
    pub const DEEP_PURPLE_600: HexColor = HexColor("#5e35b1");
    /// <span style="color:#512da8">&#9632;</span> (#512da8)
    pub const DEEP_PURPLE_700: HexColor = HexColor("#512da8");
    /// <span style="color:#4527a0">&#9632;</span> (#4527a0)
    pub const DEEP_PURPLE_800: HexColor = HexColor("#4527a0");
    /// <span style="color:#311b92">&#9632;</span> (#311b92)
    pub const DEEP_PURPLE_900: HexColor = HexColor("#311b92");
    /// <span style="color:#b388ff">&#9632;</span> (#b388ff)
    pub const DEEP_PURPLE_A100: HexColor = HexColor("#b388ff");
    /// <span style="color:#7c4dff">&#9632;</span> (#7c4dff)
    pub const DEEP_PURPLE_A200: HexColor = HexColor("#7c4dff");
    /// <span style="color:#651fff">&#9632;</span> (#651fff)
    pub const DEEP_PURPLE_A400: HexColor = HexColor("#651fff");
    /// <span style="color:#6200ea">&#9632;</span> (#6200ea)
    pub const DEEP_PURPLE_A700: HexColor = HexColor("#6200ea");

    /// <span style="color:#e8eaf6">&#9632;</span> (#e8eaf6)
    pub const INDIGO_50: HexColor = HexColor("#e8eaf6");
    /// <span style="color:#c5cae9">&#9632;</span> (#c5cae9)
    pub const INDIGO_100: HexColor = HexColor("#c5cae9");
    /// <span style="color:#9fa8da">&#9632;</span> (#9fa8da)
    pub const INDIGO_200: HexColor = HexColor("#9fa8da");
    /// <span style="color:#7986cb">&#9632;</span> (#7986cb)
    pub const INDIGO_300: HexColor = HexColor("#7986cb");
    /// <span style="color:#5c6bc0">&#9632;</span> (#5c6bc0)
    pub const INDIGO_400: HexColor = HexColor("#5c6bc0");
    /// <span style="color:#3f51b5">&#9632;</span> (#3f51b5)
    pub const INDIGO_500: HexColor = HexColor("#3f51b5");
    /// <span style="color:#3949ab">&#9632;</span> (#3949ab)
    pub const INDIGO_600: HexColor = HexColor("#3949ab");
    /// <span style="color:#303f9f">&#9632;</span> (#303f9f)
    pub const INDIGO_700: HexColor = HexColor("#303f9f");
    /// <span style="color:#283593">&#9632;</span> (#283593)
    pub const INDIGO_800: HexColor = HexColor("#283593");
    /// <span style="color:#1a237e">&#9632;</span> (#1a237e)
    pub const INDIGO_900: HexColor = HexColor("#1a237e");
    /// <span style="color:#8c9eff">&#9632;</span> (#8c9eff)
    pub const INDIGO_A100: HexColor = HexColor("#8c9eff");
    /// <span style="color:#536dfe">&#9632;</span> (#536dfe)
    pub const INDIGO_A200: HexColor = HexColor("#536dfe");
    /// <span style="color:#3d5afe">&#9632;</span> (#3d5afe)
    pub const INDIGO_A400: HexColor = HexColor("#3d5afe");
    /// <span style="color:#304ffe">&#9632;</span> (#304ffe)
    pub const INDIGO_A700: HexColor = HexColor("#304ffe");

    /// <span style="color:#e3f2fd">&#9632;</span> (#e3f2fd)
    pub const BLUE_50: HexColor = HexColor("#e3f2fd");
    /// <span style="color:#bbdefb">&#9632;</span> (#bbdefb)
    pub const BLUE_100: HexColor = HexColor("#bbdefb");
    /// <span style="color:#90caf9">&#9632;</span> (#90caf9)
    pub const BLUE_200: HexColor = HexColor("#90caf9");
    /// <span style="color:#64b5f6">&#9632;</span> (#64b5f6)
    pub const BLUE_300: HexColor = HexColor("#64b5f6");
    /// <span style="color:#42a5f5">&#9632;</span> (#42a5f5)
    pub const BLUE_400: HexColor = HexColor("#42a5f5");
    /// <span style="color:#2196f3">&#9632;</span> (#2196f3)
    pub const BLUE_500: HexColor = HexColor("#2196f3");
    /// <span style="color:#1e88e5">&#9632;</span> (#1e88e5)
    pub const BLUE_600: HexColor = HexColor("#1e88e5");
    /// <span style="color:#1976d2">&#9632;</span> (#1976d2)
    pub const BLUE_700: HexColor = HexColor("#1976d2");
    /// <span style="color:#1565c0">&#9632;</span> (#1565c0)
    pub const BLUE_800: HexColor = HexColor("#1565c0");
    /// <span style="color:#0d47a1">&#9632;</span> (#0d47a1)
    pub const BLUE_900: HexColor = HexColor("#0d47a1");
    /// <span style="color:#82b1ff">&#9632;</span> (#82b1ff)
    pub const BLUE_A100: HexColor = HexColor("#82b1ff");
    /// <span style="color:#448aff">&#9632;</span> (#448aff)
    pub const BLUE_A200: HexColor = HexColor("#448aff");
    /// <span style="color:#2979ff">&#9632;</span> (#2979ff)
    pub const BLUE_A400: HexColor = HexColor("#2979ff");
    /// <span style="color:#2962ff">&#9632;</span> (#2962ff)
    pub const BLUE_A700: HexColor = HexColor("#2962ff");

    /// <span style="color:#e1f5fe">&#9632;</span> (#e1f5fe)
    pub const LIGHT_BLUE_50: HexColor = HexColor("#e1f5fe");
    /// <span style="color:#b3e5fc">&#9632;</span> (#b3e5fc)
    pub const LIGHT_BLUE_100: HexColor = HexColor("#b3e5fc");
    /// <span style="color:#81d4fa">&#9632;</span> (#81d4fa)
    pub const LIGHT_BLUE_200: HexColor = HexColor("#81d4fa");
    /// <span style="color:#4fc3f7">&#9632;</span> (#4fc3f7)
    pub const LIGHT_BLUE_300: HexColor = HexColor("#4fc3f7");
    /// <span style="color:#29b6f6">&#9632;</span> (#29b6f6)
    pub const LIGHT_BLUE_400: HexColor = HexColor("#29b6f6");
    /// <span style="color:#03a9f4">&#9632;</span> (#03a9f4)
    pub const LIGHT_BLUE_500: HexColor = HexColor("#03a9f4");
    /// <span style="color:#039be5">&#9632;</span> (#039be5)
    pub const LIGHT_BLUE_600: HexColor = HexColor("#039be5");
    /// <span style="color:#0288d1">&#9632;</span> (#0288d1)
    pub const LIGHT_BLUE_700: HexColor = HexColor("#0288d1");
    /// <span style="color:#0277bd">&#9632;</span> (#0277bd)
    pub const LIGHT_BLUE_800: HexColor = HexColor("#0277bd");
    /// <span style="color:#01579b">&#9632;</span> (#01579b)
    pub const LIGHT_BLUE_900: HexColor = HexColor("#01579b");
    /// <span style="color:#80d8ff">&#9632;</span> (#80d8ff)
    pub const LIGHT_BLUE_A100: HexColor = HexColor("#80d8ff");
    /// <span style="color:#40c4ff">&#9632;</span> (#40c4ff)
    pub const LIGHT_BLUE_A200: HexColor = HexColor("#40c4ff");
    /// <span style="color:#00b0ff">&#9632;</span> (#00b0ff)
    pub const LIGHT_BLUE_A400: HexColor = HexColor("#00b0ff");
    /// <span style="color:#0091ea">&#9632;</span> (#0091ea)
    pub const LIGHT_BLUE_A700: HexColor = HexColor("#0091ea");

    /// <span style="color:#e0f7fa">&#9632;</span> (#e0f7fa)
    pub const CYAN_50: HexColor = HexColor("#e0f7fa");
    /// <span style="color:#b2ebf2">&#9632;</span> (#b2ebf2)
    pub const CYAN_100: HexColor = HexColor("#b2ebf2");
    /// <span style="color:#80deea">&#9632;</span> (#80deea)
    pub const CYAN_200: HexColor = HexColor("#80deea");
    /// <span style="color:#4dd0e1">&#9632;</span> (#4dd0e1)
    pub const CYAN_300: HexColor = HexColor("#4dd0e1");
    /// <span style="color:#26c6da">&#9632;</span> (#26c6da)
    pub const CYAN_400: HexColor = HexColor("#26c6da");
    /// <span style="color:#00bcd4">&#9632;</span> (#00bcd4)
    pub const CYAN_500: HexColor = HexColor("#00bcd4");
    /// <span style="color:#00acc1">&#9632;</span> (#00acc1)
    pub const CYAN_600: HexColor = HexColor("#00acc1");
    /// <span style="color:#0097a7">&#9632;</span> (#0097a7)
    pub const CYAN_700: HexColor = HexColor("#0097a7");
    /// <span style="color:#00838f">&#9632;</span> (#00838f)
    pub const CYAN_800: HexColor = HexColor("#00838f");
    /// <span style="color:#006064">&#9632;</span> (#006064)
    pub const CYAN_900: HexColor = HexColor("#006064");
    /// <span style="color:#84ffff">&#9632;</span> (#84ffff)
    pub const CYAN_A100: HexColor = HexColor("#84ffff");
    /// <span style="color:#18ffff">&#9632;</span> (#18ffff)
    pub const CYAN_A200: HexColor = HexColor("#18ffff");
    /// <span style="color:#00e5ff">&#9632;</span> (#00e5ff)
    pub const CYAN_A400: HexColor = HexColor("#00e5ff");
    /// <span style="color:#00b8d4">&#9632;</span> (#00b8d4)
    pub const CYAN_A700: HexColor = HexColor("#00b8d4");

    /// <span style="color:#e0f2f1">&#9632;</span> (#e0f2f1)
    pub const TEAL_50: HexColor = HexColor("#e0f2f1");
    /// <span style="color:#b2dfdb">&#9632;</span> (#b2dfdb)
    pub const TEAL_100: HexColor = HexColor("#b2dfdb");
    /// <span style="color:#80cbc4">&#9632;</span> (#80cbc4)
    pub const TEAL_200: HexColor = HexColor("#80cbc4");
    /// <span style="color:#4db6ac">&#9632;</span> (#4db6ac)
    pub const TEAL_300: HexColor = HexColor("#4db6ac");
    /// <span style="color:#26a69a">&#9632;</span> (#26a69a)
    pub const TEAL_400: HexColor = HexColor("#26a69a");
    /// <span style="color:#009688">&#9632;</span> (#009688)
    pub const TEAL_500: HexColor = HexColor("#009688");
    /// <span style="color:#00897b">&#9632;</span> (#00897b)
    pub const TEAL_600: HexColor = HexColor("#00897b");
    /// <span style="color:#00796b">&#9632;</span> (#00796b)
    pub const TEAL_700: HexColor = HexColor("#00796b");
    /// <span style="color:#00695c">&#9632;</span> (#00695c)
    pub const TEAL_800: HexColor = HexColor("#00695c");
    /// <span style="color:#004d40">&#9632;</span> (#004d40)
    pub const TEAL_900: HexColor = HexColor("#004d40");
    /// <span style="color:#a7ffeb">&#9632;</span> (#a7ffeb)
    pub const TEAL_A100: HexColor = HexColor("#a7ffeb");
    /// <span style="color:#64ffda">&#9632;</span> (#64ffda)
    pub const TEAL_A200: HexColor = HexColor("#64ffda");
    /// <span style="color:#1de9b6">&#9632;</span> (#1de9b6)
    pub const TEAL_A400: HexColor = HexColor("#1de9b6");
    /// <span style="color:#00bfa5">&#9632;</span> (#00bfa5)
    pub const TEAL_A700: HexColor = HexColor("#00bfa5");

    /// <span style="color:#e8f5e9">&#9632;</span> (#e8f5e9)
    pub const GREEN_50: HexColor = HexColor("#e8f5e9");
    /// <span style="color:#c8e6c9">&#9632;</span> (#c8e6c9)
    pub const GREEN_100: HexColor = HexColor("#c8e6c9");
    /// <span style="color:#a5d6a7">&#9632;</span> (#a5d6a7)
    pub const GREEN_200: HexColor = HexColor("#a5d6a7");
    /// <span style="color:#81c784">&#9632;</span> (#81c784)
    pub const GREEN_300: HexColor = HexColor("#81c784");
    /// <span style="color:#66bb6a">&#9632;</span> (#66bb6a)
    pub const GREEN_400: HexColor = HexColor("#66bb6a");
    /// <span style="color:#4caf50">&#9632;</span> (#4caf50)
    pub const GREEN_500: HexColor = HexColor("#4caf50");
    /// <span style="color:#43a047">&#9632;</span> (#43a047)
    pub const GREEN_600: HexColor = HexColor("#43a047");
    /// <span style="color:#388e3c">&#9632;</span> (#388e3c)
    pub const GREEN_700: HexColor = HexColor("#388e3c");
    /// <span style="color:#2e7d32">&#9632;</span> (#2e7d32)
    pub const GREEN_800: HexColor = HexColor("#2e7d32");
    /// <span style="color:#1b5e20">&#9632;</span> (#1b5e20)
    pub const GREEN_900: HexColor = HexColor("#1b5e20");
    /// <span style="color:#b9f6ca">&#9632;</span> (#b9f6ca)
    pub const GREEN_A100: HexColor = HexColor("#b9f6ca");
    /// <span style="color:#69f0ae">&#9632;</span> (#69f0ae)
    pub const GREEN_A200: HexColor = HexColor("#69f0ae");
    /// <span style="color:#00e676">&#9632;</span> (#00e676)
    pub const GREEN_A400: HexColor = HexColor("#00e676");
    /// <span style="color:#00c853">&#9632;</span> (#00c853)
    pub const GREEN_A700: HexColor = HexColor("#00c853");

    /// <span style="color:#f1f8e9">&#9632;</span> (#f1f8e9)
    pub const LIGHT_GREEN_50: HexColor = HexColor("#f1f8e9");
    /// <span style="color:#dcedc8">&#9632;</span> (#dcedc8)
    pub const LIGHT_GREEN_100: HexColor = HexColor("#dcedc8");
    /// <span style="color:#c5e1a5">&#9632;</span> (#c5e1a5)
    pub const LIGHT_GREEN_200: HexColor = HexColor("#c5e1a5");
    /// <span style="color:#aed581">&#9632;</span> (#aed581)
    pub const LIGHT_GREEN_300: HexColor = HexColor("#aed581");
    /// <span style="color:#9ccc65">&#9632;</span> (#9ccc65)
    pub const LIGHT_GREEN_400: HexColor = HexColor("#9ccc65");
    /// <span style="color:#8bc34a">&#9632;</span> (#8bc34a)
    pub const LIGHT_GREEN_500: HexColor = HexColor("#8bc34a");
    /// <span style="color:#7cb342">&#9632;</span> (#7cb342)
    pub const LIGHT_GREEN_600: HexColor = HexColor("#7cb342");
    /// <span style="color:#689f38">&#9632;</span> (#689f38)
    pub const LIGHT_GREEN_700: HexColor = HexColor("#689f38");
    /// <span style="color:#558b2f">&#9632;</span> (#558b2f)
    pub const LIGHT_GREEN_800: HexColor = HexColor("#558b2f");
    /// <span style="color:#33691e">&#9632;</span> (#33691e)
    pub const LIGHT_GREEN_900: HexColor = HexColor("#33691e");
    /// <span style="color:#ccff90">&#9632;</span> (#ccff90)
    pub const LIGHT_GREEN_A100: HexColor = HexColor("#ccff90");
    /// <span style="color:#b2ff59">&#9632;</span> (#b2ff59)
    pub const LIGHT_GREEN_A200: HexColor = HexColor("#b2ff59");
    /// <span style="color:#76ff03">&#9632;</span> (#76ff03)
    pub const LIGHT_GREEN_A400: HexColor = HexColor("#76ff03");
    /// <span style="color:#64dd17">&#9632;</span> (#64dd17)
    pub const LIGHT_GREEN_A700: HexColor = HexColor("#64dd17");

    /// <span style="color:#f9fbe7">&#9632;</span> (#f9fbe7)
    pub const LIME_50: HexColor = HexColor("#f9fbe7");
    /// <span style="color:#f0f4c3">&#9632;</span> (#f0f4c3)
    pub const LIME_100: HexColor = HexColor("#f0f4c3");
    /// <span style="color:#e6ee9c">&#9632;</span> (#e6ee9c)
    pub const LIME_200: HexColor = HexColor("#e6ee9c");
    /// <span style="color:#dce775">&#9632;</span> (#dce775)
    pub const LIME_300: HexColor = HexColor("#dce775");
    /// <span style="color:#d4e157">&#9632;</span> (#d4e157)
    pub const LIME_400: HexColor = HexColor("#d4e157");
    /// <span style="color:#cddc39">&#9632;</span> (#cddc39)
    pub const LIME_500: HexColor = HexColor("#cddc39");
    /// <span style="color:#c0ca33">&#9632;</span> (#c0ca33)
    pub const LIME_600: HexColor = HexColor("#c0ca33");
    /// <span style="color:#afb42b">&#9632;</span> (#afb42b)
    pub const LIME_700: HexColor = HexColor("#afb42b");
    /// <span style="color:#9e9d24">&#9632;</span> (#9e9d24)
    pub const LIME_800: HexColor = HexColor("#9e9d24");
    /// <span style="color:#827717">&#9632;</span> (#827717)
    pub const LIME_900: HexColor = HexColor("#827717");
    /// <span style="color:#f4ff81">&#9632;</span> (#f4ff81)
    pub const LIME_A100: HexColor = HexColor("#f4ff81");
    /// <span style="color:#eeff41">&#9632;</span> (#eeff41)
    pub const LIME_A200: HexColor = HexColor("#eeff41");
    /// <span style="color:#c6ff00">&#9632;</span> (#c6ff00)
    pub const LIME_A400: HexColor = HexColor("#c6ff00");
    /// <span style="color:#aeea00">&#9632;</span> (#aeea00)
    pub const LIME_A700: HexColor = HexColor("#aeea00");

    /// <span style="color:#fffde7">&#9632;</span> (#fffde7)
    pub const YELLOW_50: HexColor = HexColor("#fffde7");
    /// <span style="color:#fff9c4">&#9632;</span> (#fff9c4)
    pub const YELLOW_100: HexColor = HexColor("#fff9c4");
    /// <span style="color:#fff59d">&#9632;</span> (#fff59d)
    pub const YELLOW_200: HexColor = HexColor("#fff59d");
    /// <span style="color:#fff176">&#9632;</span> (#fff176)
    pub const YELLOW_300: HexColor = HexColor("#fff176");
    /// <span style="color:#ffee58">&#9632;</span> (#ffee58)
    pub const YELLOW_400: HexColor = HexColor("#ffee58");
    /// <span style="color:#ffeb3b">&#9632;</span> (#ffeb3b)
    pub const YELLOW_500: HexColor = HexColor("#ffeb3b");
    /// <span style="color:#fdd835">&#9632;</span> (#fdd835)
    pub const YELLOW_600: HexColor = HexColor("#fdd835");
    /// <span style="color:#fbc02d">&#9632;</span> (#fbc02d)
    pub const YELLOW_700: HexColor = HexColor("#fbc02d");
    /// <span style="color:#f9a825">&#9632;</span> (#f9a825)
    pub const YELLOW_800: HexColor = HexColor("#f9a825");
    /// <span style="color:#f57f17">&#9632;</span> (#f57f17)
    pub const YELLOW_900: HexColor = HexColor("#f57f17");
    /// <span style="color:#ffff8d">&#9632;</span> (#ffff8d)
    pub const YELLOW_A100: HexColor = HexColor("#ffff8d");
    /// <span style="color:#ffff00">&#9632;</span> (#ffff00)
    pub const YELLOW_A200: HexColor = HexColor("#ffff00");
    /// <span style="color:#ffea00">&#9632;</span> (#ffea00)
    pub const YELLOW_A400: HexColor = HexColor("#ffea00");
    /// <span style="color:#ffd600">&#9632;</span> (#ffd600)
    pub const YELLOW_A700: HexColor = HexColor("#ffd600");

    /// <span style="color:#fff8e1">&#9632;</span> (#fff8e1)
    pub const AMBER_50: HexColor = HexColor("#fff8e1");
    /// <span style="color:#ffecb3">&#9632;</span> (#ffecb3)
    pub const AMBER_100: HexColor = HexColor("#ffecb3");
    /// <span style="color:#ffe082">&#9632;</span> (#ffe082)
    pub const AMBER_200: HexColor = HexColor("#ffe082");
    /// <span style="color:#ffd54f">&#9632;</span> (#ffd54f)
    pub const AMBER_300: HexColor = HexColor("#ffd54f");
    /// <span style="color:#ffca28">&#9632;</span> (#ffca28)
    pub const AMBER_400: HexColor = HexColor("#ffca28");
    /// <span style="color:#ffc107">&#9632;</span> (#ffc107)
    pub const AMBER_500: HexColor = HexColor("#ffc107");
    /// <span style="color:#ffb300">&#9632;</span> (#ffb300)
    pub const AMBER_600: HexColor = HexColor("#ffb300");
    /// <span style="color:#ffa000">&#9632;</span> (#ffa000)
    pub const AMBER_700: HexColor = HexColor("#ffa000");
    /// <span style="color:#ff8f00">&#9632;</span> (#ff8f00)
    pub const AMBER_800: HexColor = HexColor("#ff8f00");
    /// <span style="color:#ff6f00">&#9632;</span> (#ff6f00)
    pub const AMBER_900: HexColor = HexColor("#ff6f00");
    /// <span style="color:#ffe57f">&#9632;</span> (#ffe57f)
    pub const AMBER_A100: HexColor = HexColor("#ffe57f");
    /// <span style="color:#ffd740">&#9632;</span> (#ffd740)
    pub const AMBER_A200: HexColor = HexColor("#ffd740");
    /// <span style="color:#ffc400">&#9632;</span> (#ffc400)
    pub const AMBER_A400: HexColor = HexColor("#ffc400");
    /// <span style="color:#ffab00">&#9632;</span> (#ffab00)
    pub const AMBER_A700: HexColor = HexColor("#ffab00");

    /// <span style="color:#fff3e0">&#9632;</span> (#fff3e0)
    pub const ORANGE_50: HexColor = HexColor("#fff3e0");
    /// <span style="color:#ffe0b2">&#9632;</span> (#ffe0b2)
    pub const ORANGE_100: HexColor = HexColor("#ffe0b2");
    /// <span style="color:#ffcc80">&#9632;</span> (#ffcc80)
    pub const ORANGE_200: HexColor = HexColor("#ffcc80");
    /// <span style="color:#ffb74d">&#9632;</span> (#ffb74d)
    pub const ORANGE_300: HexColor = HexColor("#ffb74d");
    /// <span style="color:#ffa726">&#9632;</span> (#ffa726)
    pub const ORANGE_400: HexColor = HexColor("#ffa726");
    /// <span style="color:#ff9800">&#9632;</span> (#ff9800)
    pub const ORANGE_500: HexColor = HexColor("#ff9800");
    /// <span style="color:#fb8c00">&#9632;</span> (#fb8c00)
    pub const ORANGE_600: HexColor = HexColor("#fb8c00");
    /// <span style="color:#f57c00">&#9632;</span> (#f57c00)
    pub const ORANGE_700: HexColor = HexColor("#f57c00");
    /// <span style="color:#ef6c00">&#9632;</span> (#ef6c00)
    pub const ORANGE_800: HexColor = HexColor("#ef6c00");
    /// <span style="color:#e65100">&#9632;</span> (#e65100)
    pub const ORANGE_900: HexColor = HexColor("#e65100");
    /// <span style="color:#ffd180">&#9632;</span> (#ffd180)
    pub const ORANGE_A100: HexColor = HexColor("#ffd180");
    /// <span style="color:#ffab40">&#9632;</span> (#ffab40)
    pub const ORANGE_A200: HexColor = HexColor("#ffab40");
    /// <span style="color:#ff9100">&#9632;</span> (#ff9100)
    pub const ORANGE_A400: HexColor = HexColor("#ff9100");
    /// <span style="color:#ff6d00">&#9632;</span> (#ff6d00)
    pub const ORANGE_A700: HexColor = HexColor("#ff6d00");

    /// <span style="color:#fbe9e7">&#9632;</span> (#fbe9e7)
    pub const DEEP_ORANGE_50: HexColor = HexColor("#fbe9e7");
    /// <span style="color:#ffccbc">&#9632;</span> (#ffccbc)
    pub const DEEP_ORANGE_100: HexColor = HexColor("#ffccbc");
    /// <span style="color:#ffab91">&#9632;</span> (#ffab91)
    pub const DEEP_ORANGE_200: HexColor = HexColor("#ffab91");
    /// <span style="color:#ff8a65">&#9632;</span> (#ff8a65)
    pub const DEEP_ORANGE_300: HexColor = HexColor("#ff8a65");
    /// <span style="color:#ff7043">&#9632;</span> (#ff7043)
    pub const DEEP_ORANGE_400: HexColor = HexColor("#ff7043");
    /// <span style="color:#ff5722">&#9632;</span> (#ff5722)
    pub const DEEP_ORANGE_500: HexColor = HexColor("#ff5722");
    /// <span style="color:#f4511e">&#9632;</span> (#f4511e)
    pub const DEEP_ORANGE_600: HexColor = HexColor("#f4511e");
    /// <span style="color:#e64a19">&#9632;</span> (#e64a19)
    pub const DEEP_ORANGE_700: HexColor = HexColor("#e64a19");
    /// <span style="color:#d84315">&#9632;</span> (#d84315)
    pub const DEEP_ORANGE_800: HexColor = HexColor("#d84315");
    /// <span style="color:#bf360c">&#9632;</span> (#bf360c)
    pub const DEEP_ORANGE_900: HexColor = HexColor("#bf360c");
    /// <span style="color:#ff9e80">&#9632;</span> (#ff9e80)
    pub const DEEP_ORANGE_A100: HexColor = HexColor("#ff9e80");
    /// <span style="color:#ff6e40">&#9632;</span> (#ff6e40)
    pub const DEEP_ORANGE_A200: HexColor = HexColor("#ff6e40");
    /// <span style="color:#ff3d00">&#9632;</span> (#ff3d00)
    pub const DEEP_ORANGE_A400: HexColor = HexColor("#ff3d00");
    /// <span style="color:#dd2c00">&#9632;</span> (#dd2c00)
    pub const DEEP_ORANGE_A700: HexColor = HexColor("#dd2c00");

    /// <span style="color:#efebe9">&#9632;</span> (#efebe9)
    pub const BROWN_50: HexColor = HexColor("#efebe9");
    /// <span style="color:#d7ccc8">&#9632;</span> (#d7ccc8)
    pub const BROWN_100: HexColor = HexColor("#d7ccc8");
    /// <span style="color:#bcaaa4">&#9632;</span> (#bcaaa4)
    pub const BROWN_200: HexColor = HexColor("#bcaaa4");
    /// <span style="color:#a1887f">&#9632;</span> (#a1887f)
    pub const BROWN_300: HexColor = HexColor("#a1887f");
    /// <span style="color:#8d6e63">&#9632;</span> (#8d6e63)
    pub const BROWN_400: HexColor = HexColor("#8d6e63");
    /// <span style="color:#795548">&#9632;</span> (#795548)
    pub const BROWN_500: HexColor = HexColor("#795548");
    /// <span style="color:#6d4c41">&#9632;</span> (#6d4c41)
    pub const BROWN_600: HexColor = HexColor("#6d4c41");
    /// <span style="color:#5d4037">&#9632;</span> (#5d4037)
    pub const BROWN_700: HexColor = HexColor("#5d4037");
    /// <span style="color:#4e342e">&#9632;</span> (#4e342e)
    pub const BROWN_800: HexColor = HexColor("#4e342e");
    /// <span style="color:#3e2723">&#9632;</span> (#3e2723)
    pub const BROWN_900: HexColor = HexColor("#3e2723");

    /// <span style="color:#fafafa">&#9632;</span> (#fafafa)
    pub const GREY_50: HexColor = HexColor("#fafafa");
    /// <span style="color:#f5f5f5">&#9632;</span> (#f5f5f5)
    pub const GREY_100: HexColor = HexColor("#f5f5f5");
    /// <span style="color:#eeeeee">&#9632;</span> (#eeeeee)
    pub const GREY_200: HexColor = HexColor("#eeeeee");
    /// <span style="color:#e0e0e0">&#9632;</span> (#e0e0e0)
    pub const GREY_300: HexColor = HexColor("#e0e0e0");
    /// <span style="color:#bdbdbd">&#9632;</span> (#bdbdbd)
    pub const GREY_400: HexColor = HexColor("#bdbdbd");
    /// <span style="color:#9e9e9e">&#9632;</span> (#9e9e9e)
    pub const GREY_500: HexColor = HexColor("#9e9e9e");
    /// <span style="color:#757575">&#9632;</span> (#757575)
    pub const GREY_600: HexColor = HexColor("#757575");
    /// <span style="color:#616161">&#9632;</span> (#616161)
    pub const GREY_700: HexColor = HexColor("#616161");
    /// <span style="color:#424242">&#9632;</span> (#424242)
    pub const GREY_800: HexColor = HexColor("#424242");
    /// <span style="color:#212121">&#9632;</span> (#212121)
    pub const GREY_900: HexColor = HexColor("#212121");

    /// <span style="color:#eceff1">&#9632;</span> (#eceff1)
    pub const BLUE_GREY_50: HexColor = HexColor("#eceff1");
    /// <span style="color:#cfd8dc">&#9632;</span> (#cfd8dc)
    pub const BLUE_GREY_100: HexColor = HexColor("#cfd8dc");
    /// <span style="color:#b0bec5">&#9632;</span> (#b0bec5)
    pub const BLUE_GREY_200: HexColor = HexColor("#b0bec5");
    /// <span style="color:#90a4ae">&#9632;</span> (#90a4ae)
    pub const BLUE_GREY_300: HexColor = HexColor("#90a4ae");
    /// <span style="color:#78909c">&#9632;</span> (#78909c)
    pub const BLUE_GREY_400: HexColor = HexColor("#78909c");
    /// <span style="color:#607d8b">&#9632;</span> (#607d8b)
    pub const BLUE_GREY_500: HexColor = HexColor("#607d8b");
    /// <span style="color:#546e7a">&#9632;</span> (#546e7a)
    pub const BLUE_GREY_600: HexColor = HexColor("#546e7a");
    /// <span style="color:#455a64">&#9632;</span> (#455a64)
    pub const BLUE_GREY_700: HexColor = HexColor("#455a64");
    /// <span style="color:#37474f">&#9632;</span> (#37474f)
    pub const BLUE_GREY_800: HexColor = HexColor("#37474f");
    /// <span style="color:#263238">&#9632;</span> (#263238)
    pub const BLUE_GREY_900: HexColor = HexColor("#263238");

    /// <span style="color:#000000">&#9632;</span> (#000000)
    pub const BLACK: HexColor = HexColor("#000000");
    /// <span style="color:#ffffff">&#9632;</span> (#ffffff)
    pub const WHITE: HexColor = HexColor("#ffffff");

    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;
        use rstest::rstest;

        #[rstest]
        #[case::valid_hex(Some(HexColor("#ff0000")), "#ff0000")]
        #[case::valid_name(Some(RED_500), "red_500")]
        #[case::invalid_name(None, "red_5")]
        #[case::invalid_hex(None, "#ff00g0")]
        #[case::invalid_hex_no_hash(None, "ff0000")]
        #[case::invalid_hex_too_short(None, "#ff00")]
        #[case::invalid_hex_too_long(None, "#ff00000")]
        #[case::invalid_hex_no_hash_too_short(None, "ff00")]
        #[case::invalid_hex_no_hash_too_long(None, "ff00000")]
        fn test_parse(#[case] expected: Option<HexColor>, #[case] input: &'static str) {
            let actual = HexColor::parse(input);
            assert_eq!(actual, expected);
        }
    }
}
