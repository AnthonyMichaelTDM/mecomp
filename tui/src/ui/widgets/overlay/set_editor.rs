//! Set editor overlay for editing a list of string values with inline text boxes

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::ui::widgets::input_box::{InputBox, InputBoxState};
use crate::ui::{AppState, components::ComponentRender};
use crate::{
    state::action::{Action, OverlayAction},
    ui::widgets::overlay::OverlayResult,
};
use tokio::sync::mpsc::UnboundedSender;

use super::Overlay;

/// Set editor overlay for managing a list of items with inline editing
#[derive(Clone, Debug)]
pub struct SetEditorOverlay {
    pub target_id: u64,
    pub size: Rect,
    /// Input boxes for each item
    pub item_inputs: Vec<InputBoxState>,
    /// Index of the currently selected input (or `item_inputs.len()` for Add button)
    pub selected_index: usize,
    /// Scroll offset for vertical scrolling
    pub scroll_offset: usize,
    /// Cached visible items from last render (for scroll calculations in key handlers)
    cached_visible_items: usize,
}

impl PartialEq for SetEditorOverlay {
    fn eq(&self, other: &Self) -> bool {
        self.target_id == other.target_id
            && self.selected_index == other.selected_index
            && self.scroll_offset == other.scroll_offset
            && self.item_inputs.len() == other.item_inputs.len()
    }
}

impl Eq for SetEditorOverlay {}

impl SetEditorOverlay {
    /// Create a new set editor overlay from a list of strings
    #[must_use]
    pub fn new(target_id: u64, items: Vec<String>, width: u16) -> Self {
        let item_inputs = items
            .into_iter()
            .map(|item| {
                let mut input = InputBoxState::new();
                input.set_text(&item);
                input
            })
            .collect();

        Self {
            target_id,
            size: Rect::new(0, 0, width, 15), // 15 rows for title + items + add button + help
            item_inputs,
            selected_index: 0,
            scroll_offset: 0,
            cached_visible_items: 10, // Default; updated each render
        }
    }

    const fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
        // Adjust scroll if selected goes above the visible area
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    const fn move_down(&mut self, visible_items: usize) {
        let max_index = self.item_inputs.len(); // can select items or "Add" button
        if self.selected_index < max_index {
            self.selected_index += 1;
            // Adjust scroll if selected goes below the visible area
            if self.selected_index >= self.scroll_offset + visible_items {
                self.scroll_offset = self.selected_index - visible_items + 1;
            }
        }
    }

    fn add_new_item(&mut self) {
        self.item_inputs.push(InputBoxState::new());
        self.selected_index = self.item_inputs.len() - 1;
    }

    fn delete_current(&mut self) {
        if self.selected_index < self.item_inputs.len() {
            self.item_inputs.remove(self.selected_index);
            if self.selected_index > 0 && self.selected_index >= self.item_inputs.len() {
                self.selected_index -= 1;
            }
        }
    }

    /// Get the final items as strings, filtering out empty ones
    fn get_items(&self) -> Vec<String> {
        self.item_inputs
            .iter()
            .map(|input| input.text().to_string())
            .collect()
    }
}

impl Overlay for SetEditorOverlay {
    fn area(&self, terminal_area: Rect) -> Rect {
        let layout = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(self.size.width),
            Constraint::Fill(1),
        ]);
        let [_, horizontal_area, _] = terminal_area.layout(&layout);

        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(self.size.height),
            Constraint::Fill(1),
        ]);
        let [_, area, _] = horizontal_area.layout(&layout);

        area
    }

    fn update_with_state(&mut self, _: &AppState) {}

    fn inner_handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>) {
        // If on Add button
        if self.selected_index == self.item_inputs.len() {
            match key.code {
                KeyCode::Up => self.move_up(),
                KeyCode::Enter => {
                    self.add_new_item();
                }
                // Can't go further down
                _ => {}
            }
        } else if self.selected_index < self.item_inputs.len() {
            // Editing an item
            match key.code {
                KeyCode::Up => self.move_up(),
                KeyCode::Down => self.move_down(self.cached_visible_items),
                KeyCode::Delete => self.delete_current(),
                KeyCode::Enter => {
                    // Confirm the entire set and close
                    action_tx
                        .send(Action::Overlay(OverlayAction::Commit(
                            OverlayResult::SetEdited {
                                target_id: self.target_id,
                                items: self.get_items(),
                            },
                        )))
                        .ok();
                }
                _ => {
                    // Delegate to input box for text editing
                    self.item_inputs[self.selected_index].handle_key_event(key);
                }
            }
        }
    }

    fn inner_handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        _action_tx: UnboundedSender<Action>,
    ) {
        use crossterm::event::{MouseButton, MouseEventKind};

        // Calculate the inner area (excluding border) the same way render_border does
        let block = Block::bordered();
        let inner_area = block.inner(area);

        let MouseEvent {
            kind, row, column, ..
        } = mouse;
        let position = Position::new(column, row);

        // Handle scrolling
        match kind {
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                return;
            }
            MouseEventKind::ScrollDown => {
                let num_items = self.item_inputs.len();
                let visible_items = (inner_area.height - 1) as usize;
                if self.scroll_offset + visible_items < num_items {
                    self.scroll_offset += 1;
                }
                return;
            }
            _ => {}
        }

        // Only handle left click
        if kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }

        // Check if click is within the inner area
        if !inner_area.contains(position) {
            return;
        }

        // Calculate which row was clicked (relative to inner_area.y)
        let click_row = row.saturating_sub(inner_area.y);

        let num_items = self.item_inputs.len();
        let visible_items = (inner_area.height - 1) as usize;

        // Check if clicking on an item
        if (click_row as usize) < visible_items {
            let actual_idx = self.scroll_offset + (click_row as usize);

            if actual_idx < num_items {
                // Select this item
                self.selected_index = actual_idx;

                // Pass click to InputBox handler for cursor positioning
                // The input box was rendered at:
                // - x: inner_area.x + 2 (after the "N: " label)
                // - y: inner_area.y + click_row
                // - width: inner_area.width - 2
                // - height: 1
                let input_area = Rect {
                    x: inner_area.x + 2,
                    y: inner_area.y + click_row,
                    width: inner_area.width.saturating_sub(2),
                    height: 1,
                };

                self.item_inputs[actual_idx].handle_mouse_event(mouse, input_area);
                return;
            }
        }

        // Check if clicking on the [Add] button
        let rendered_items = (num_items - self.scroll_offset).min(visible_items);
        if (click_row as usize) == rendered_items {
            self.selected_index = num_items; // Select Add button
        }
    }
}

impl ComponentRender<Rect> for SetEditorOverlay {
    fn render_border(&mut self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        let block = Block::bordered()
            .title_top("Edit Set")
            .title_bottom("↑/↓: navigate | Del: remove item | Enter: confirm/save | Esc: cancel");
        frame.render_widget(&block, area);
        block.inner(area)
    }

    fn render_content(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if area.height < 3 {
            return;
        }
        let num_items = self.item_inputs.len();
        let visible_items = (area.height - 1) as usize; // Leave room for [Add] button

        // Cache visible items for scroll calculations in key handlers
        self.cached_visible_items = visible_items;

        // Calculate how much space we have for items
        let mut constraints = Vec::new();
        for _ in 0..visible_items {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(1)); // [Add] button
        constraints.push(Constraint::Fill(1)); // Filler

        let layout = Layout::vertical(&constraints);
        let sections = layout.split(area);

        // Render each item, accounting for scroll offset
        let mut rendered_items = 0;
        for display_idx in 0..visible_items {
            let actual_idx = self.scroll_offset + display_idx;

            if actual_idx >= num_items {
                break;
            }

            if display_idx >= sections.len() {
                break;
            }

            rendered_items = display_idx + 1;
            let input = &mut self.item_inputs[actual_idx];
            let is_selected = actual_idx == self.selected_index;
            let section = sections[display_idx];

            if is_selected {
                // Active item: show as editable input box with label prefix
                // Create a custom layout for the label and input
                let item_layout = Layout::horizontal([
                    Constraint::Length(3), // For "N: "
                    Constraint::Fill(1),   // For the input
                ]);
                let [label_area, input_area] = item_layout.areas(section);

                // Render label
                let label_text = format!("{}:", actual_idx + 1);
                frame.render_widget(Paragraph::new(label_text), label_area);

                // Render input box
                let style = Style::default().add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                let input_box = InputBox::new().style(style);
                frame.render_stateful_widget(input_box, input_area, input);

                // Set cursor position
                if input_area.width > 0 {
                    let cursor_pos = input_area + input.cursor_offset();
                    frame.set_cursor_position(cursor_pos);
                }
            } else {
                // Inactive item: show as static text
                let label = format!("{}: {}", actual_idx + 1, input.text());
                let style = Style::default();
                let line = Line::from(Span::styled(label, style));
                frame.render_widget(Paragraph::new(line), section);
            }
        }

        // Render [Add] button right after the last rendered item
        let add_button_idx = rendered_items;
        if add_button_idx < sections.len() {
            let add_section = sections[add_button_idx];
            let is_selected = self.selected_index == num_items;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let add_line = Line::from(Span::styled("[Add]", style));
            frame.render_widget(Paragraph::new(add_line), add_section);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::buffer::Buffer;

    use crate::test_utils::setup_test_terminal;

    // -- constants ------------------------------------------------------------

    /// A 40x15 area matching the default overlay size, positioned at (0,0).
    /// inner_area = Rect { x:1, y:1, width:38, height:13 }, visible_items = 12.
    const OVERLAY_AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 15,
    };

    // -- helpers --------------------------------------------------------------

    fn make_overlay(items: &[&str], selected: usize, scroll: usize) -> SetEditorOverlay {
        let mut o = SetEditorOverlay::new(1, items.iter().map(|s| s.to_string()).collect(), 40);
        o.selected_index = selected;
        o.scroll_offset = scroll;
        o
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

    fn row_str(buf: &Buffer, row: u16) -> String {
        (0..buf.area().width)
            .map(|col| buf.cell((col, row)).unwrap().symbol().to_owned())
            .collect()
    }

    // -- initialisation (existing) --------------------------------------------

    #[test]
    fn set_editor_new_initializes() {
        let overlay = SetEditorOverlay::new(1, vec!["a".to_string(), "b".to_string()], 40);
        assert_eq!(overlay.item_inputs.len(), 2);
        assert_eq!(overlay.target_id, 1);
    }

    #[test]
    fn set_editor_move_down() {
        let mut overlay = SetEditorOverlay::new(1, vec!["a".to_string(), "b".to_string()], 40);
        assert_eq!(overlay.selected_index, 0);
        overlay.move_down(10);
        assert_eq!(overlay.selected_index, 1);
        overlay.move_down(10);
        assert_eq!(overlay.selected_index, 2); // Add button
    }

    #[test]
    fn set_editor_add_new_item() {
        let mut overlay = SetEditorOverlay::new(1, vec!["a".to_string()], 40);
        assert_eq!(overlay.item_inputs.len(), 1);
        overlay.add_new_item();
        assert_eq!(overlay.item_inputs.len(), 2);
        assert_eq!(overlay.selected_index, 1);
    }

    #[test]
    fn set_editor_delete_item() {
        let mut overlay = SetEditorOverlay::new(1, vec!["a".to_string(), "b".to_string()], 40);
        overlay.delete_current();
        assert_eq!(overlay.item_inputs.len(), 1);
        assert_eq!(overlay.item_inputs[0].text(), "b");
    }

    #[test]
    fn set_editor_delete_last_item() {
        let mut overlay = SetEditorOverlay::new(1, vec!["a".to_string()], 40);
        overlay.delete_current();
        assert!(overlay.item_inputs.is_empty());
    }

    #[test]
    fn set_editor_get_items() {
        let mut overlay = SetEditorOverlay::new(1, vec!["a".to_string(), "b".to_string()], 40);
        overlay.item_inputs[0].set_text("x");
        overlay.item_inputs[1].set_text("y");
        let items = overlay.get_items();
        assert_eq!(items, vec!["x".to_string(), "y".to_string()]);
    }

    // -- move_up --------------------------------------------------------------

    #[test]
    fn move_up_at_zero_stays() {
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
    fn move_up_adjusts_scroll_offset() {
        // scroll=2, selected=2 -> move_up -> selected=1 < scroll(2) -> scroll = selected = 1
        let mut o = make_overlay(&["A", "B", "C", "D"], 2, 2);
        o.move_up();
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn move_up_from_add_button_goes_to_last_item() {
        // selected_index == len() == 2 (Add button)
        let mut o = make_overlay(&["A", "B"], 2, 0);
        o.move_up();
        assert_eq!(o.selected_index, 1);
    }

    // -- move_down ------------------------------------------------------------

    #[test]
    fn move_down_reaches_add_button() {
        // selected at last item -> move_down should land on Add button
        let mut o = make_overlay(&["A", "B"], 1, 0);
        o.move_down(10);
        assert_eq!(o.selected_index, 2); // len() == 2
    }

    #[test]
    fn move_down_at_add_button_stays() {
        // selected_index == len() -> move_down is a no-op
        let mut o = make_overlay(&["A", "B"], 2, 0);
        o.move_down(10);
        assert_eq!(o.selected_index, 2);
    }

    #[test]
    fn move_down_adjusts_scroll_offset() {
        // 3 items, visible=2, selected=1 at scroll=0
        // moving down -> selected=2, 2 >= 0+2 -> scroll = 2-2+1 = 1
        let mut o = make_overlay(&["A", "B", "C"], 1, 0);
        o.move_down(2);
        assert_eq!(o.selected_index, 2);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn move_down_within_visible_window_no_scroll_change() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        o.move_down(10);
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 0);
    }

    // -- inner_handle_key_event -----------------------------------------------

    #[test]
    fn key_up_navigates_up() {
        let mut o = make_overlay(&["A", "B"], 1, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Up), tx);
        assert_eq!(o.selected_index, 0);
    }

    #[test]
    fn key_down_navigates_down() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Down), tx);
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn enter_on_item_sends_set_edited_action() {
        let mut o = make_overlay(&["hello", "world"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Enter), tx);
        let action = rx.blocking_recv().expect("expected action");
        assert_eq!(
            action,
            Action::Overlay(OverlayAction::Commit(OverlayResult::SetEdited {
                target_id: 1,
                items: vec!["hello".to_string(), "world".to_string()],
            }))
        );
    }

    #[test]
    fn enter_on_add_button_adds_item() {
        // selected_index == len() == 1 -> Enter adds new item
        let mut o = make_overlay(&["A"], 1, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Enter), tx);
        assert_eq!(o.item_inputs.len(), 2);
    }

    #[test]
    fn up_on_add_button_moves_to_last_item() {
        let mut o = make_overlay(&["A", "B"], 2, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Up), tx);
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn delete_key_removes_current_item() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Delete), tx);
        assert_eq!(o.item_inputs.len(), 1);
        assert_eq!(o.item_inputs[0].text(), "B");
    }

    #[test]
    fn char_key_delegates_to_input_box() {
        let mut o = make_overlay(&[""], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Char('z')), tx);
        assert!(
            o.item_inputs[0].text().contains('z'),
            "expected 'z' in input, got {:?}",
            o.item_inputs[0].text()
        );
    }

    #[test]
    fn char_key_ignored_on_add_button() {
        // When the Add button is selected, char input is silently dropped
        let mut o = make_overlay(&["A"], 1, 0);
        let initial_len = o.item_inputs.len();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_key_event(key(KeyCode::Char('z')), tx);
        assert_eq!(o.item_inputs.len(), initial_len, "no item should be added");
        assert!(rx.try_recv().is_err(), "no action should be sent");
    }

    // -- handle_key_event (Overlay wrapper) -----------------------------------

    #[test]
    fn esc_sends_close_action() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        Overlay::handle_key_event(&mut o, key(KeyCode::Esc), tx);
        let action = rx.blocking_recv().expect("expected close action");
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn release_event_is_ignored() {
        let mut o = make_overlay(&["A", "B"], 1, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        Overlay::handle_key_event(
            &mut o,
            key_with_kind(KeyCode::Up, KeyEventKind::Release),
            tx,
        );
        assert_eq!(o.selected_index, 1, "state must not change for Release");
        assert!(rx.try_recv().is_err(), "no action expected for Release");
    }

    #[test]
    fn handle_key_event_delegates_to_inner() {
        let mut o = make_overlay(&["A", "B", "C"], 2, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        Overlay::handle_key_event(&mut o, key(KeyCode::Up), tx);
        assert_eq!(o.selected_index, 1);
    }

    // -- inner_handle_mouse_event ---------------------------------------------

    #[test]
    fn scroll_up_decrements_scroll_offset() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 2);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollUp, 5, 5), OVERLAY_AREA, tx);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn scroll_up_at_zero_stays() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollUp, 5, 5), OVERLAY_AREA, tx);
        assert_eq!(o.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_increments_scroll_offset() {
        // inner_area height = 13, visible_items = 12; need >12 items to scroll.
        // With 20 items and scroll_offset=0: 0+12 < 20 -> can scroll.
        let items: Vec<String> = (0..20).map(|i| format!("item{i}")).collect();
        let mut o = SetEditorOverlay::new(1, items, 40);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollDown, 5, 5), OVERLAY_AREA, tx);
        assert_eq!(o.scroll_offset, 1);
    }

    #[test]
    fn scroll_down_at_max_stays() {
        // 2 items, visible_items=12: scroll_offset(0)+12 >= 2 -> cannot scroll further
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::ScrollDown, 5, 5), OVERLAY_AREA, tx);
        assert_eq!(o.scroll_offset, 0);
    }

    #[test]
    fn left_click_selects_item() {
        // inner_area = Rect { x:1, y:1, width:38, height:13 }
        // click at (col=5, row=2) -> click_row = 2-1 = 1 -> actual_idx = 0+1 = 1
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
            OVERLAY_AREA,
            tx,
        );
        assert_eq!(o.selected_index, 1);
    }

    #[test]
    fn left_click_on_add_button_selects_add() {
        // 2 items, scroll=0, rendered_items=2
        // Add button is at inner row 2 -> absolute row 3 (inner_area.y=1 + 2 = 3)
        // click_row = 3-1 = 2; actual_idx=2 >= num_items(2) -> no item selected
        // rendered_items = (2-0).min(12) = 2; click_row(2) == rendered_items(2) -> Add!
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 5, 3),
            OVERLAY_AREA,
            tx,
        );
        assert_eq!(o.selected_index, 2); // num_items == Add button index
    }

    #[test]
    fn left_click_on_border_is_ignored() {
        // Border cells are outside inner_area -> click is a no-op
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let original_index = o.selected_index;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 0, 0),
            OVERLAY_AREA,
            tx,
        );
        assert_eq!(o.selected_index, original_index, "state must not change");
        assert!(rx.try_recv().is_err(), "no action expected");
    }

    #[test]
    fn non_left_non_scroll_mouse_event_is_ignored() {
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        o.inner_handle_mouse_event(mouse(MouseEventKind::Moved, 5, 2), OVERLAY_AREA, tx);
        assert_eq!(o.selected_index, 0, "state must not change");
        assert!(rx.try_recv().is_err(), "no action expected");
    }

    // -- handle_mouse_event (Overlay wrapper) ---------------------------------

    #[test]
    fn click_outside_overlay_sends_close() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 35);
        let overlay_area = o.area(terminal_area);
        // (0,0) is well outside the centred overlay
        let click = mouse(MouseEventKind::Down(MouseButton::Left), 0, 0);
        Overlay::handle_mouse_event(&mut o, click, overlay_area, tx);
        let action = rx.blocking_recv().expect("expected close action");
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn scroll_outside_overlay_does_not_close() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 35);
        let overlay_area = o.area(terminal_area);
        let ev = mouse(MouseEventKind::ScrollUp, 0, 0);
        Overlay::handle_mouse_event(&mut o, ev, overlay_area, tx);
        assert!(
            rx.try_recv().is_err(),
            "scroll outside should not send Close"
        );
    }

    #[test]
    fn click_inside_overlay_delegates_to_inner() {
        let mut o = make_overlay(&["A", "B", "C"], 0, 0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal_area = Rect::new(0, 0, 80, 35);
        let overlay_area = o.area(terminal_area);
        // Click at inner row 1 of the overlay (overlay_area.y+2 lands inside inner_area)
        let click = mouse(
            MouseEventKind::Down(MouseButton::Left),
            overlay_area.x + 5,
            overlay_area.y + 2,
        );
        Overlay::handle_mouse_event(&mut o, click, overlay_area, tx);
        // Row 2 relative to overlay top -> click_row = 2-1 = 1 -> item index 1
        assert_eq!(o.selected_index, 1);
    }

    // -- area() ---------------------------------------------------------------

    #[test]
    fn area_centers_overlay_in_terminal() {
        // size = 40x15; terminal = 80x35
        // x = (80-40)/2 = 20 (exact), y = (35-15)/2 = 10 (exact)
        let o = SetEditorOverlay::new(1, vec![], 40);
        let terminal_area = Rect::new(0, 0, 80, 35);
        let a = o.area(terminal_area);
        assert_eq!(a.width, 40);
        assert_eq!(a.height, 15);
        assert_eq!(a.x, 20);
        assert_eq!(a.y, 10);
    }

    // -- update_with_state ----------------------------------------------------

    #[test]
    fn update_with_state_is_noop() {
        let mut o = make_overlay(&["A", "B"], 1, 2);
        let state = AppState::default();
        o.update_with_state(&state);
        assert_eq!(o.selected_index, 1);
        assert_eq!(o.scroll_offset, 2);
    }

    // -- rendering ------------------------------------------------------------

    #[test]
    fn render_border_shows_title() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                o.render_border(frame, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let row0 = row_str(&buf, 0);
        assert!(
            row0.contains("Edit Set"),
            "expected 'Edit Set' in row 0, got: {row0:?}"
        );
    }

    #[test]
    fn render_border_shows_help_text() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                o.render_border(frame, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let last_row = row_str(&buf, 14);
        assert!(
            last_row.contains("↑") || last_row.contains("navigate"),
            "expected navigation hint in bottom border, got: {last_row:?}"
        );
    }

    #[test]
    fn render_shows_unselected_item_text() {
        // item 1 (index 1, unselected) shows "2: bar"
        let mut o = make_overlay(&["foo", "bar"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                let inner = o.render_border(frame, area);
                o.render_content(frame, inner);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        // inner_area.y = 1; item[1] renders at inner row 1 -> absolute row 2
        let row2 = row_str(&buf, 2);
        assert!(
            row2.contains("2: bar"),
            "expected '2: bar' in row 2, got: {row2:?}"
        );
    }

    #[test]
    fn render_shows_selected_item_label() {
        // selected item (index 0) shows with "1:" label prefix
        let mut o = make_overlay(&["foo", "bar"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                let inner = o.render_border(frame, area);
                o.render_content(frame, inner);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let row1 = row_str(&buf, 1);
        assert!(
            row1.contains("1:"),
            "expected '1:' label in row 1, got: {row1:?}"
        );
    }

    #[test]
    fn render_shows_add_button() {
        // 2 items -> Add button is at inner row 2 -> absolute row 3
        let mut o = make_overlay(&["foo", "bar"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                let inner = o.render_border(frame, area);
                o.render_content(frame, inner);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let row3 = row_str(&buf, 3);
        assert!(
            row3.contains("[Add]"),
            "expected '[Add]' in row 3, got: {row3:?}"
        );
    }

    #[test]
    fn render_content_skips_when_area_too_small() {
        // height < 3 -> render_content is a no-op (no panic)
        let mut o = make_overlay(&["A", "B"], 0, 0);
        let (mut terminal, _) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                o.render_content(
                    frame,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 2,
                    },
                );
            })
            .unwrap();
        // No assertion needed - success means no panic
    }

    #[test]
    fn render_has_border_corners() {
        let mut o = make_overlay(&["A"], 0, 0);
        let (mut terminal, area) = setup_test_terminal(40, 15);
        terminal
            .draw(|frame| {
                o.render_border(frame, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "┌");
        assert_eq!(buf.cell((39, 0)).unwrap().symbol(), "┐");
        assert_eq!(buf.cell((0, 14)).unwrap().symbol(), "└");
        assert_eq!(buf.cell((39, 14)).unwrap().symbol(), "┘");
    }
}
