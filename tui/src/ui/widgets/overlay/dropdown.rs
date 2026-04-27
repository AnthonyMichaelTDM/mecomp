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
use crate::{
    state::action::{Action, OverlayAction},
    ui::widgets::overlay::OverlayResult,
};
use tokio::sync::mpsc::UnboundedSender;

use super::Overlay;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropdownOverlay {
    pub target_id: u64,
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

    fn inner_handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>) {
        match key.code {
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Enter => {
                action_tx
                    .send(Action::Overlay(OverlayAction::Commit(
                        OverlayResult::DropdownSelected {
                            target_id: self.target_id,
                            selected_index: self.selected_index,
                        },
                    )))
                    .ok();
            }
            _ => {}
        }
    }

    fn inner_handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: UnboundedSender<Action>,
    ) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let y = mouse.row.saturating_sub(area.y);
                let clicked_index = self.scroll_offset + usize::from(y);
                if clicked_index < self.options.len() {
                    self.selected_index = clicked_index;
                    action_tx
                        .send(Action::Overlay(OverlayAction::Commit(
                            OverlayResult::DropdownSelected {
                                target_id: self.target_id,
                                selected_index: self.selected_index,
                            },
                        )))
                        .ok();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

    #[test]
    fn enter_commits_selected_index() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut overlay = DropdownOverlay {
            target_id: 7,
            area: Rect::new(10, 10, 8, 3),
            options,
            selected_index: 1,
            scroll_offset: 0,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Enter), tx);

        let action = rx.blocking_recv().expect("expected commit action");
        assert_eq!(
            action,
            Action::Overlay(OverlayAction::Commit(OverlayResult::DropdownSelected {
                target_id: 7,
                selected_index: 1,
            }))
        );
    }

    #[test]
    fn click_commits_clicked_index() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut overlay = DropdownOverlay {
            target_id: 3,
            area: Rect::new(10, 10, 8, 3),
            options,
            selected_index: 0,
            scroll_offset: 0,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 11,
            row: 11,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };

        overlay.inner_handle_mouse_event(click, overlay.area, tx);

        let action = rx.blocking_recv().expect("expected commit action");
        assert_eq!(
            action,
            Action::Overlay(OverlayAction::Commit(OverlayResult::DropdownSelected {
                target_id: 3,
                selected_index: 1,
            }))
        );
    }
}
