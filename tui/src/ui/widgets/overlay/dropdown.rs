use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
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
    pub size: Rect,
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

        let visible_rows = usize::from(self.size.height.max(1));
        let max_visible_index = self.scroll_offset + visible_rows.saturating_sub(1);
        if self.selected_index > max_visible_index {
            self.scroll_offset = self.selected_index + 1 - visible_rows;
        }
    }
}

impl Overlay for DropdownOverlay {
    fn area(&self, terminal_area: Rect) -> Rect {
        // place the overlay in the middle of the terminal
        terminal_area.centered(
            Constraint::Length(self.size.width),
            Constraint::Length(self.size.height),
        )
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
    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    use crate::test_utils::{assert_buffer_eq, setup_test_terminal};
    use ratatui::buffer::Buffer;

    // -- helpers --------------------------------------------------------------

    fn make_overlay(options: &[&str], selected: usize, scroll: usize) -> DropdownOverlay {
        DropdownOverlay {
            target_id: 1,
            size: Rect::new(0, 0, 10, 4),
            options: Arc::from(options.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            selected_index: selected,
            scroll_offset: scroll,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn key_with_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::empty(), kind)
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    // -- move_up --------------------------------------------------------------

    #[test]
    fn move_up_from_zero_stays_at_zero() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        o.move_up();
        assert_eq!(o.selected_index, 0);
        assert_eq!(o.scroll_offset, 0);
    }

    #[test]
    fn move_up_decrements_selected_index() {
        let mut o = make_overlay(&["A", "B", "C"], 2, 0);
        o.move_up();
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn move_up_clamps_scroll_offset_down() {
        // scroll_offset=2, selected_index=2; after move_up selected=1 < scroll_offset -> clamp
        let mut o = make_overlay(&["A", "B", "C", "D"], 2, 2);
        o.move_up();
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn move_up_empty_options_resets_to_zero() {
        let mut o = make_overlay(&[], 0, 0);
        o.move_up();
        assert_eq!(o.selected_index, 0);
        assert_eq!(o.scroll_offset, 0);
    }

    // -- move_down ------------------------------------------------------------

    #[test]
    fn move_down_at_last_index_stays_at_last() {
        let mut o = make_overlay(&["A", "B", "C"], 2, 0);
        o.move_down();
        assert_eq!(o.selected_index, 2);
    }

    #[test]
    fn move_down_increments_selected_index() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        o.move_down();
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn move_down_within_visible_window_no_scroll_change() {
        // size.height=4, so visible_rows=4; selected 0->1 stays within window
        let mut o = make_overlay(&["A", "B", "C", "D", "E"], 0, 0);
        o.move_down();
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 0);
    }

    #[test]
    fn move_down_adjusts_scroll_offset_when_past_visible_window() {
        // size.height=4 means visible_rows=4; scroll_offset=0, selected at 3 -> move to 4
        // max_visible_index = 0 + 3 = 3; selected(4) > 3 -> scroll_offset = 4+1-4 = 1
        let mut o = make_overlay(&["A", "B", "C", "D", "E"], 3, 0);
        o.move_down();
        assert_eq!(o.selected_index, 4);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn move_down_empty_options_resets_to_zero() {
        let mut o = make_overlay(&[], 0, 0);
        o.move_down();
        assert_eq!(o.selected_index, 0);
        assert_eq!(o.scroll_offset, 0);
    }

    // -- inner_handle_key_event -----------------------------------------------

    #[test]
    fn key_up_calls_move_up() {
        let mut o = make_overlay(&["A", "B"], 1, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Up), tx);
        assert_eq!(o.selected_index, 0);
    }

    #[test]
    fn key_down_calls_move_down() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Down), tx);
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn unknown_key_sends_no_action() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Tab), tx);
        assert!(rx.try_recv().is_err(), "no action expected for Tab");
    }

    #[test]
    fn enter_commits_selected_index() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut overlay = DropdownOverlay {
            target_id: 7,
            size: Rect::new(10, 10, 8, 3),
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

    // -- handle_key_event (Overlay wrapper) -----------------------------------

    #[test]
    fn esc_sends_close_action() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // handle_key_event is the Overlay-trait wrapper; call via trait
        Overlay::handle_key_event(&mut o, key(KeyCode::Esc), tx);
        let action = rx.blocking_recv().expect("expected close action");
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn non_press_key_event_is_ignored() {
        let mut o = make_overlay(&["A", "B"], 1, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // Release event - should be ignored entirely
        Overlay::handle_key_event(
            &mut o,
            key_with_kind(KeyCode::Up, KeyEventKind::Release),
            tx,
        );
        // selected_index must NOT have changed
        assert_eq!(o.selected_index, 1);
        assert!(rx.try_recv().is_err(), "no action expected for Release");
    }

    #[test]
    fn handle_key_event_delegates_up_to_inner() {
        let mut o = make_overlay(&["A", "B", "C"], 2, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        Overlay::handle_key_event(&mut o, key(KeyCode::Up), tx);
        assert_eq!(o.selected_index, 1);
    }

    // -- inner_handle_mouse_event ---------------------------------------------

    #[test]
    fn click_commits_clicked_index() {
        let options: Arc<[String]> = Arc::from(vec!["A".to_string(), "B".to_string()]);
        let mut overlay = DropdownOverlay {
            target_id: 3,
            size: Rect::new(10, 10, 8, 3),
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

        overlay.inner_handle_mouse_event(click, overlay.size, tx);

        let action = rx.blocking_recv().expect("expected commit action");
        assert_eq!(
            action,
            Action::Overlay(OverlayAction::Commit(OverlayResult::DropdownSelected {
                target_id: 3,
                selected_index: 1,
            }))
        );
    }

    #[test]
    fn click_out_of_bounds_index_sends_no_action() {
        // Click on row past the last option
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let area = Rect::new(0, 0, 10, 4);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // row=5 -> y=5, clicked_index = 0+5 = 5 >= options.len(2)
        let click = mouse(MouseEventKind::Down(MouseButton::Left), 1, 5);
        o.inner_handle_mouse_event(click, area, tx);
        assert!(
            rx.try_recv().is_err(),
            "click past options should send no action"
        );
    }

    #[test]
    fn scroll_up_event_calls_move_up() {
        let mut o = make_overlay(&["A", "B", "C"], 2, 0);
        let area = Rect::new(0, 0, 10, 4);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollUp, 1, 1), area, tx);
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn scroll_down_event_calls_move_down() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        let area = Rect::new(0, 0, 10, 4);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollDown, 1, 1), area, tx);
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn other_mouse_event_sends_no_action_and_no_state_change() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let area = Rect::new(0, 0, 10, 4);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::Moved, 1, 1), area, tx);
        assert_eq!(o.selected_index, 0, "state should not change");
        assert!(rx.try_recv().is_err(), "no action expected for Moved");
    }

    // -- handle_mouse_event (Overlay wrapper) ---------------------------------

    #[test]
    fn out_of_area_left_click_sends_close() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 24);
        // area() centers size(10x4) in 80x24: x=35, y=10 - (0,0) is outside
        let overlay_area = o.area(terminal_area);
        let click = mouse(MouseEventKind::Down(MouseButton::Left), 0, 0);
        Overlay::handle_mouse_event(&mut o, click, overlay_area, tx);
        let action = rx.blocking_recv().expect("expected close action");
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn out_of_area_non_left_click_sends_no_action() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 24);
        let overlay_area = o.area(terminal_area);
        // Scroll at (0,0) is outside the centered overlay -> no action
        let ev = mouse(MouseEventKind::ScrollUp, 0, 0);
        Overlay::handle_mouse_event(&mut o, ev, overlay_area, tx);
        assert!(rx.try_recv().is_err(), "no action expected");
    }

    #[test]
    fn in_area_click_delegates_to_inner() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 24);
        let overlay_area = o.area(terminal_area);
        // Click row 1 inside overlay (area.y + 1) -> clicked_index = scroll_offset(0) + 1 = 1
        let click = mouse(
            MouseEventKind::Down(MouseButton::Left),
            overlay_area.x + 1,
            overlay_area.y + 1,
        );
        Overlay::handle_mouse_event(&mut o, click, overlay_area, tx);
        let action = rx.blocking_recv().expect("expected commit action");
        assert_eq!(
            action,
            Action::Overlay(OverlayAction::Commit(OverlayResult::DropdownSelected {
                target_id: 1,
                selected_index: 1,
            }))
        );
    }

    // -- area() ---------------------------------------------------------------

    #[test]
    fn area_centers_in_terminal() {
        let o = make_overlay(&[], 0, 0); // size = 10x4
        let terminal_area = Rect::new(0, 0, 80, 24);
        let a = o.area(terminal_area);
        // centered: x = (80-10)/2 = 35, y = (24-4)/2 = 10
        assert_eq!(a.width, 10);
        assert_eq!(a.height, 4);
        assert_eq!(a.x, 35);
        assert_eq!(a.y, 10);
    }

    // -- update_with_state ----------------------------------------------------

    #[test]
    fn update_with_state_is_noop() {
        let mut o = make_overlay(&["A", "B"], 1, 0);
        let state = AppState::default();
        o.update_with_state(&state);
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 0);
    }

    // -- rendering ------------------------------------------------------------

    #[test]
    fn render_content_shows_correct_text() {
        let mut o = make_overlay(&["Alpha", "Beta"], 0, 0);
        // Use a 10x4 terminal so the area matches size exactly
        let (mut terminal, area) = setup_test_terminal(10, 4);
        let buffer = terminal
            .draw(|frame| o.render_content(frame, area))
            .unwrap()
            .buffer
            .clone();

        // Block::bordered() draws a border; inner area is 8x2
        // Row 0: top border ┌────────┐
        // Row 1:            │Alpha   │  (selected, REVERSED)
        // Row 2:            │Beta    │
        // Row 3:            └────────┘
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌────────┐",
            "│Alpha   │",
            "│Beta    │",
            "└────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn render_content_scroll_shows_correct_items() {
        // scroll_offset=1 -> only "Beta" and "Gamma" visible
        let mut o = make_overlay(&["Alpha", "Beta", "Gamma"], 1, 1);
        let (mut terminal, area) = setup_test_terminal(10, 4);
        let buffer = terminal
            .draw(|frame| o.render_content(frame, area))
            .unwrap()
            .buffer
            .clone();

        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌────────┐",
            "│Beta    │",
            "│Gamma   │",
            "└────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn render_content_has_border() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(10, 4);
        terminal
            .draw(|frame| o.render_content(frame, area))
            .unwrap();

        let buf = terminal.backend().buffer().clone();
        // Top-left corner should be the border character '┌'
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "┌");
        // Top-right corner
        assert_eq!(buf.cell((9, 0)).unwrap().symbol(), "┐");
        // Bottom-left corner
        assert_eq!(buf.cell((0, 3)).unwrap().symbol(), "└");
        // Bottom-right corner
        assert_eq!(buf.cell((9, 3)).unwrap().symbol(), "┘");
    }

    #[test]
    fn render_selected_item_has_reversed_modifier() {
        let mut o = make_overlay(&["Alpha", "Beta"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(10, 4);
        terminal
            .draw(|frame| o.render_content(frame, area))
            .unwrap();

        let buf = terminal.backend().buffer().clone();
        // Row 1 is the first option (selected_index=0), col 1 is inside the border
        let cell = buf.cell((1, 1)).unwrap();
        assert!(
            cell.modifier.contains(Modifier::REVERSED),
            "selected item should have REVERSED modifier, got {:?}",
            cell.modifier
        );

        // Row 2 is the second option (not selected), should NOT be reversed
        let cell2 = buf.cell((1, 2)).unwrap();
        assert!(
            !cell2.modifier.contains(Modifier::REVERSED),
            "unselected item should NOT have REVERSED modifier"
        );
    }
}
