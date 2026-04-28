//! Set editor overlay for editing a list of string values with inline text boxes

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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
    /// Index of the currently selected input (or `item_inputs.len()` for Save button)
    pub selected_index: usize,
    /// Scroll offset for vertical scrolling
    pub scroll_offset: usize,
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
        }
    }

    fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
        self.scroll_offset = self.scroll_offset.min(self.selected_index);
    }

    fn move_down(&mut self) {
        let max_index = self.item_inputs.len(); // can select items or "Add" button
        if self.selected_index < max_index {
            self.selected_index += 1;
            let visible_rows = usize::from(self.size.height.saturating_sub(4).max(1));
            if self.selected_index > visible_rows && self.selected_index > self.scroll_offset {
                self.scroll_offset = self.selected_index - visible_rows;
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
                KeyCode::Down => self.move_down(),
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
        _mouse: MouseEvent,
        _area: Rect,
        _action_tx: UnboundedSender<Action>,
    ) {
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
        overlay.move_down();
        assert_eq!(overlay.selected_index, 1);
        overlay.move_down();
        assert_eq!(overlay.selected_index, 2); // Save button
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
}
