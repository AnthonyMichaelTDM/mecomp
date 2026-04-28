use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

use super::overlay::{OverlayResult, OverlayType, dropdown::DropdownOverlay};

#[derive(Debug, Clone)]
pub struct DropdownState<T> {
    control_id: u64,
    options: Arc<[String]>,
    selected_index: usize,
    scroll_offset: usize,
    is_open: bool,
    option_type: std::marker::PhantomData<T>,
    widest_option_length: usize,
}

impl<T: ToString> DropdownState<T> {
    #[must_use]
    pub fn new(control_id: u64, options: impl IntoIterator<Item = T>) -> Self {
        let options: Arc<[String]> = options
            .into_iter()
            .map(|option| option.to_string())
            .collect::<Vec<_>>()
            .into();
        let widest_option_length = options.iter().map(|s| s.chars().count()).max().unwrap_or(0);
        Self {
            control_id,
            options,
            selected_index: 0,
            scroll_offset: 0,
            is_open: false,
            option_type: std::marker::PhantomData,
            widest_option_length,
        }
    }

    #[must_use]
    pub const fn control_id(&self) -> u64 {
        self.control_id
    }

    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.selected_index
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.is_open
    }

    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(String::as_str)
    }

    pub fn set_selected_index(&mut self, selected_index: usize) {
        if self.options.is_empty() {
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }

        self.selected_index = selected_index.min(self.options.len() - 1);
        self.scroll_offset = self.scroll_offset.min(self.selected_index);
    }

    #[must_use]
    pub fn select_by_text(&mut self, value: &str) -> bool {
        if let Some(index) = self.options.iter().position(|item| item == value) {
            self.set_selected_index(index);
            return true;
        }

        false
    }

    pub const fn close_overlay(&mut self) {
        self.is_open = false;
    }

    #[must_use]
    pub fn open_overlay(&mut self, max_rows: u16) -> OverlayType {
        self.is_open = true;
        let width = u16::try_from(self.widest_option_length + 2).unwrap_or(u16::MAX);
        let height = max_rows.min(u16::try_from(self.options.len()).unwrap_or(u16::MAX));
        let size = Rect::new(0, 0, width, height);

        OverlayType::Dropdown(DropdownOverlay {
            target_id: self.control_id,
            size,
            options: self.options.clone(),
            selected_index: self.selected_index,
            scroll_offset: self.scroll_offset,
        })
    }

    pub fn apply_overlay_result(&mut self, result: &OverlayResult) -> bool {
        match result {
            OverlayResult::DropdownSelected {
                target_id,
                selected_index,
            } if *target_id == self.control_id => {
                if self.options.is_empty() {
                    self.selected_index = 0;
                } else {
                    self.selected_index = (*selected_index).min(self.options.len() - 1);
                }
                self.close_overlay();
                true
            }
            _ => false,
        }
    }
}

pub struct Dropdown<'a, T> {
    style: Style,
    open_indicator: &'a str,
    closed_indicator: &'a str,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> Dropdown<'a, T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            open_indicator: "▲",
            closed_indicator: "▼",
            _phantom: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub const fn open_indicator(mut self, indicator: &'a str) -> Self {
        self.open_indicator = indicator;
        self
    }

    #[must_use]
    pub const fn closed_indicator(mut self, indicator: &'a str) -> Self {
        self.closed_indicator = indicator;
        self
    }
}

impl<T> Default for Dropdown<'_, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: std::fmt::Display> StatefulWidget for Dropdown<'_, T> {
    type State = DropdownState<T>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let label = state
            .selected()
            .map(ToString::to_string)
            .unwrap_or_default();

        let indicator = if state.is_open() {
            self.open_indicator
        } else {
            self.closed_indicator
        };

        // Truncate to available width minus indicator + space
        let max_label = (area.width as usize).saturating_sub(2);
        let truncated: String = label.chars().take(max_label).collect();

        let line = Line::from(vec![
            Span::raw("["),
            Span::styled(truncated, self.style),
            Span::raw(" "),
            Span::raw(indicator),
            Span::raw("]"),
        ]);

        line.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use ratatui::widgets::StatefulWidget;
    use strum::IntoEnumIterator;

    use super::*;

    #[derive(Debug, Clone, PartialEq, strum::EnumIter)]
    enum Color {
        Red,
        Green,
        Blue,
    }
    impl Display for Color {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Red => write!(f, "Red"),
                Self::Green => write!(f, "Green"),
                Self::Blue => write!(f, "Blue"),
            }
        }
    }

    fn make_state() -> DropdownState<Color> {
        let options = Color::iter();
        DropdownState::new(0, options)
    }

    #[test]
    fn open_overlay_marks_open_and_places_below_if_space() {
        let mut state = make_state();
        let overlay = state.open_overlay(6);

        assert!(state.is_open());
        let OverlayType::Dropdown(dropdown) = overlay;
        assert_eq!(dropdown.size, Rect::new(2, 3, 10, 3));
        assert_eq!(dropdown.target_id, 0);
    }

    #[test]
    fn apply_overlay_result_updates_when_target_matches() {
        let mut state = make_state();
        let _overlay = state.open_overlay(5);

        let changed = state.apply_overlay_result(&OverlayResult::DropdownSelected {
            target_id: 0,
            selected_index: 1,
        });

        assert!(changed);
        assert_eq!(state.selected_index(), 1);
        assert!(!state.is_open());
    }

    #[test]
    fn apply_overlay_result_ignores_other_targets() {
        let mut state = make_state();

        let changed = state.apply_overlay_result(&OverlayResult::DropdownSelected {
            target_id: 2,
            selected_index: 1,
        });

        assert!(!changed);
        assert_eq!(state.selected_index(), 0);
    }

    #[test]
    fn widget_render_smoke() {
        let mut state = make_state();
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 3));

        Dropdown::new().render(Rect::new(0, 0, 16, 3), &mut buf, &mut state);

        assert_eq!(buf.area, Rect::new(0, 0, 16, 3));
    }

    #[test]
    fn select_by_text_updates_selected_index() {
        let mut state = make_state();

        let changed = state.select_by_text("Green");

        assert!(changed);
        assert_eq!(state.selected_index(), 1);
        assert_eq!(state.selected(), Some("Green"));
    }
}
