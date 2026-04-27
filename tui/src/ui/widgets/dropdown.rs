use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};

use super::overlay::{OverlayResult, OverlayType, dropdown::DropdownOverlay};

#[derive(Debug, Clone)]
pub struct DropdownState {
    control_id: u64,
    options: Arc<[String]>,
    selected_index: usize,
    scroll_offset: usize,
    is_open: bool,
}

impl DropdownState {
    #[must_use]
    pub fn new(control_id: u64, options: impl Into<Arc<[String]>>) -> Self {
        let options = options.into();
        Self {
            control_id,
            options,
            selected_index: 0,
            scroll_offset: 0,
            is_open: false,
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
    pub fn selected_option(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(String::as_str)
    }

    pub const fn close_overlay(&mut self) {
        self.is_open = false;
    }

    #[must_use]
    pub fn open_overlay(
        &mut self,
        anchor: Rect,
        terminal_area: Rect,
        max_rows: u16,
    ) -> OverlayType {
        self.is_open = true;
        let area = compute_overlay_area(anchor, terminal_area, self.options.len(), max_rows);

        OverlayType::Dropdown(DropdownOverlay {
            target_id: self.control_id,
            area,
            options: Arc::clone(&self.options),
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

fn compute_overlay_area(anchor: Rect, terminal: Rect, option_count: usize, max_rows: u16) -> Rect {
    let width = anchor.width.min(terminal.width.max(1));
    let x = anchor.x.min(terminal.right().saturating_sub(width));

    let desired_rows = u16::try_from(option_count)
        .unwrap_or(u16::MAX)
        .max(1)
        .min(max_rows.max(1));

    let below_y = anchor.bottom();
    let below_space = terminal.bottom().saturating_sub(below_y);

    if below_space >= desired_rows {
        return Rect::new(x, below_y, width, desired_rows);
    }

    let above_space = anchor.y.saturating_sub(terminal.y);
    let height = desired_rows.min(above_space.max(1));
    let y = anchor.y.saturating_sub(height);

    Rect::new(x, y, width, height)
}

pub struct Dropdown<'a> {
    focused: bool,
    placeholder: &'a str,
}

impl<'a> Dropdown<'a> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            placeholder: "Select...",
        }
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }
}

impl Default for Dropdown<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Dropdown<'_> {
    type State = DropdownState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let text = state.selected_option().unwrap_or(self.placeholder);
        let style = if self.focused {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        Paragraph::new(text)
            .style(style)
            .block(Block::bordered())
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::widgets::StatefulWidget;

    use super::*;

    #[test]
    fn open_overlay_marks_open_and_places_below_if_space() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut state = DropdownState::new(9, options);
        let overlay = state.open_overlay(Rect::new(2, 2, 10, 1), Rect::new(0, 0, 30, 20), 6);

        assert!(state.is_open());
        let OverlayType::Dropdown(dropdown) = overlay;
        assert_eq!(dropdown.area, Rect::new(2, 3, 10, 2));
        assert_eq!(dropdown.target_id, 9);
    }

    #[test]
    fn apply_overlay_result_updates_when_target_matches() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut state = DropdownState::new(1, options);
        let _overlay = state.open_overlay(Rect::new(0, 0, 10, 1), Rect::new(0, 0, 30, 10), 5);

        let changed = state.apply_overlay_result(&OverlayResult::DropdownSelected {
            target_id: 1,
            selected_index: 1,
        });

        assert!(changed);
        assert_eq!(state.selected_index(), 1);
        assert!(!state.is_open());
    }

    #[test]
    fn apply_overlay_result_ignores_other_targets() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut state = DropdownState::new(1, options);

        let changed = state.apply_overlay_result(&OverlayResult::DropdownSelected {
            target_id: 2,
            selected_index: 1,
        });

        assert!(!changed);
        assert_eq!(state.selected_index(), 0);
    }

    #[test]
    fn widget_render_smoke() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string()]);
        let mut state = DropdownState::new(1, options);
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 3));

        Dropdown::new()
            .focused(true)
            .render(Rect::new(0, 0, 16, 3), &mut buf, &mut state);

        assert_eq!(buf.area, Rect::new(0, 0, 16, 3));
    }
}
