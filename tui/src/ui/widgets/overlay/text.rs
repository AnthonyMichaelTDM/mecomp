//! Text input overlay for editing values in query builder

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::Block,
};

use crate::ui::widgets::input_box::{InputBox, InputBoxState};
use crate::{
    state::action::{Action, OverlayAction},
    ui::{AppState, components::ComponentRender},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{Overlay, OverlayResult};

/// Represents the type of value being edited (for validation/display hints)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// Plain text (title, album, etc.)
    Text,
    /// Integer year value
    Integer,
    /// Single item in a set (building a comma-separated list)
    SetItem,
}

/// Text input overlay for editing query values.
/// Reuses `InputBoxState` for consistent text editing behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextOverlay {
    pub target_id: u64,
    pub size: Rect,
    /// The text input widget state
    pub input: InputBoxState,
    /// What kind of value is being edited (for validation hints)
    pub value_kind: ValueKind,
}

impl TextOverlay {
    /// Create a new text overlay
    #[must_use]
    pub fn new(target_id: u64, initial_text: &str, value_kind: ValueKind, width: u16) -> Self {
        let mut input = InputBoxState::new();
        input.set_text(initial_text);
        Self {
            target_id,
            size: Rect::new(0, 0, width, 3), // 3 rows: prompt + input + hints
            input,
            value_kind,
        }
    }
}

impl Overlay for TextOverlay {
    fn area(&self, terminal_area: Rect) -> Rect {
        // place the overlay in the middle of the terminal
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
        match key.code {
            KeyCode::Esc => {
                // Cancelled without saving
                action_tx.send(Action::Overlay(OverlayAction::Close)).ok();
            }
            KeyCode::Enter => {
                // Commit the text
                action_tx
                    .send(Action::Overlay(OverlayAction::Commit(
                        OverlayResult::TextInputted {
                            target_id: self.target_id,
                            text: self.input.text().to_string(),
                        },
                    )))
                    .ok();
            }
            _ => {
                // Delegate to input box for text editing
                self.input.handle_key_event(key);
            }
        }
    }

    fn inner_handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        _action_tx: UnboundedSender<Action>,
    ) {
        // Defer to input box for mouse click positioning
        self.input.handle_mouse_event(mouse, area);
    }
}

impl ComponentRender<Rect> for TextOverlay {
    fn render_border(&mut self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        // Render the border
        let block = Block::bordered()
            .title_top("Edit Value")
            .title_bottom(match self.value_kind {
                ValueKind::Text => "Enter text | Esc: cancel | Enter: save",
                ValueKind::Integer => "Enter number | Esc: cancel | Enter: save",
                ValueKind::SetItem => "Enter item | Esc: cancel | Enter: save",
            });
        frame.render_widget(&block, area);
        block.inner(area)
    }

    fn render_content(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if area.height < 1 {
            return;
        }

        // Create InputBox widget without border (border is rendered separately by render_border)
        let input_box = InputBox::new();

        // Render the InputBox as a StatefulWidget
        // This automatically handles:
        // - Updating scroll to keep cursor visible
        // - Calculating proper cursor_offset based on inner_area
        // - Rendering with horizontal scroll
        frame.render_stateful_widget(input_box, area, &mut self.input);

        // Set cursor position using the area + cursor_offset
        // The cursor_offset is calculated relative to the area we just rendered to
        let cursor_pos = area + self.input.cursor_offset();
        frame.set_cursor_position(cursor_pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_overlay_new_initializes_with_text() {
        let overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        assert_eq!(overlay.input.text(), "hello");
        assert_eq!(overlay.target_id, 1);
    }

    #[test]
    fn text_overlay_accepts_text_input() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(crossterm::event::KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(overlay.input.text(), "a");
    }

    #[test]
    fn integer_overlay_still_accepts_all_input_from_input_box() {
        // Note: InputBoxState doesn't validate, so we validate in apply_text_result instead
        let overlay = TextOverlay::new(1, "123", ValueKind::Integer, 20);
        assert_eq!(overlay.input.text(), "123");
    }

    #[test]
    fn text_overlay_with_set_item() {
        let overlay = TextOverlay::new(1, "genre1", ValueKind::SetItem, 20);
        assert_eq!(overlay.input.text(), "genre1");
    }
}
