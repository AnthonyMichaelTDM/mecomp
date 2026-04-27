use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState},
};
use std::sync::Arc;

use crate::ui::{AppState, components::ComponentRender};

use super::Overlay;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropdownOverlay {
    pub area: Rect,
    pub options: Arc<[String]>,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl DropdownOverlay {
    fn move_up(&mut self) {
        if self.options.is_empty() {
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }

        self.selected_index = self.selected_index.saturating_sub(1);
        self.scroll_offset = self.scroll_offset.min(self.selected_index);
    }

    fn move_down(&mut self) {
        if self.options.is_empty() {
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }

        let last = self.options.len().saturating_sub(1);
        self.selected_index = (self.selected_index + 1).min(last);

        let visible_rows = usize::from(self.area.height.max(1));
        let max_visible_index = self.scroll_offset + visible_rows.saturating_sub(1);
        if self.selected_index > max_visible_index {
            self.scroll_offset = self.selected_index + 1 - visible_rows;
        }
    }
}

impl Overlay for DropdownOverlay {
    fn area(&self, _: Rect) -> Rect {
        self.area
    }

    fn update_with_state(&mut self, _: &AppState) {}

    fn inner_handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            _ => {}
        }
    }

    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let y = mouse.row.saturating_sub(area.y);
                let clicked_index = self.scroll_offset + usize::from(y);
                if clicked_index < self.options.len() {
                    self.selected_index = clicked_index;
                }
            }
            MouseEventKind::ScrollUp => self.move_up(),
            MouseEventKind::ScrollDown => self.move_down(),
            _ => {}
        }
    }
}

impl ComponentRender<Rect> for DropdownOverlay {
    fn render_border(&mut self, _frame: &mut Frame<'_>, area: Rect) -> Rect {
        area
    }

    fn render_content(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let visible = usize::from(area.height);
        let items = self
            .options
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible)
            .map(|(idx, value)| {
                let style = if idx == self.selected_index {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(value.clone(), style)))
            })
            .collect::<Vec<_>>();

        let list = List::new(items).block(Block::bordered());
        let mut state = ListState::default();
        if self.selected_index >= self.scroll_offset {
            let rel = self.selected_index - self.scroll_offset;
            if rel < visible {
                state.select(Some(rel));
            }
        }

        frame.render_stateful_widget(list, area, &mut state);
    }
}
