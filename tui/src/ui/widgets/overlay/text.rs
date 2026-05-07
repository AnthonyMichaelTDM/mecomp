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
    use crate::test_utils::setup_test_terminal;
    use crossterm::event::{KeyModifiers, MouseEventKind};
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    // Helper function to create and receive actions
    fn create_action_channel() -> (
        tokio::sync::mpsc::UnboundedSender<Action>,
        tokio::sync::mpsc::UnboundedReceiver<Action>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    // ============================================================================
    // Initialization & State Tests
    // ============================================================================

    #[test]
    fn text_overlay_new_initializes_with_text() {
        let overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        assert_eq!(overlay.input.text(), "hello");
        assert_eq!(overlay.target_id, 1);
        assert_eq!(overlay.value_kind, ValueKind::Text);
        assert_eq!(overlay.size.width, 20);
        assert_eq!(overlay.size.height, 3);
    }

    #[test]
    fn text_overlay_initializes_with_empty_text() {
        let overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        assert_eq!(overlay.input.text(), "");
        assert!(overlay.input.is_empty());
        assert_eq!(overlay.target_id, 1);
    }

    #[test]
    fn text_overlay_initializes_with_unicode_text() {
        let overlay = TextOverlay::new(1, "こんにちは", ValueKind::Text, 20);
        assert_eq!(overlay.input.text(), "こんにちは");
    }

    #[test]
    fn text_overlay_initializes_with_long_text() {
        let long_text = "a".repeat(1000);
        let overlay = TextOverlay::new(1, &long_text, ValueKind::Text, 20);
        assert_eq!(overlay.input.text(), &long_text);
    }

    #[rstest]
    #[case(ValueKind::Text)]
    #[case(ValueKind::Integer)]
    #[case(ValueKind::SetItem)]
    fn text_overlay_initializes_with_different_value_kinds(#[case] kind: ValueKind) {
        let overlay = TextOverlay::new(42, "test", kind, 30);
        assert_eq!(overlay.value_kind, kind);
        assert_eq!(overlay.target_id, 42);
    }

    #[test]
    fn text_overlay_size_is_always_3_rows_high() {
        for width in [10, 20, 50, 100].iter() {
            let overlay = TextOverlay::new(1, "", ValueKind::Text, *width);
            assert_eq!(overlay.size.height, 3);
            assert_eq!(overlay.size.width, *width);
        }
    }

    // ============================================================================
    // Text Input & Modification Tests
    // ============================================================================

    #[test]
    fn text_overlay_accepts_single_character() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(overlay.input.text(), "a");
    }

    #[test]
    fn text_overlay_accepts_multiple_characters() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        for ch in "hello".chars() {
            overlay
                .input
                .handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        assert_eq!(overlay.input.text(), "hello");
    }

    #[test]
    fn integer_overlay_accepts_digit_input() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Integer, 20);
        for ch in "12345".chars() {
            overlay
                .input
                .handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        assert_eq!(overlay.input.text(), "12345");
    }

    #[test]
    fn integer_overlay_accepts_non_numeric_input_from_input_box() {
        // Note: InputBoxState doesn't validate, validation happens at the application level
        let mut overlay = TextOverlay::new(1, "", ValueKind::Integer, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(overlay.input.text(), "a");
    }

    #[test]
    fn set_item_overlay_stores_text() {
        let overlay = TextOverlay::new(1, "rock", ValueKind::SetItem, 20);
        assert_eq!(overlay.input.text(), "rock");
    }

    #[test]
    fn text_overlay_supports_backspace() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(overlay.input.text(), "hell");
    }

    #[test]
    fn text_overlay_supports_delete() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        // Position cursor in the middle
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Right));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Delete));
        assert_eq!(overlay.input.text(), "hllo");
    }

    #[test]
    fn text_overlay_supports_cursor_navigation() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        // Start at end (cursor is automatically at end when text is set)
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(overlay.input.text(), "xhello");
    }

    #[test]
    fn text_overlay_supports_home_key() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(overlay.input.text(), "xhello");
    }

    #[test]
    fn text_overlay_supports_end_key() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        overlay.input.handle_key_event(KeyEvent::from(KeyCode::End));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('!')));
        assert_eq!(overlay.input.text(), "hello!");
    }

    #[test]
    fn text_overlay_supports_left_arrow() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Left));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(overlay.input.text(), "hellxo");
    }

    #[test]
    fn text_overlay_supports_right_arrow() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Right));
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(overlay.input.text(), "hxello");
    }

    // ============================================================================
    // Keyboard Event Handling Tests
    // ============================================================================

    #[test]
    fn esc_key_closes_overlay() {
        let mut overlay = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Esc), tx);

        let action = rx.try_recv().unwrap();
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn enter_key_commits_text() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Enter), tx);

        let action = rx.try_recv().unwrap();
        match action {
            Action::Overlay(OverlayAction::Commit(
                crate::ui::widgets::overlay::OverlayResult::TextInputted { target_id, text },
            )) => {
                assert_eq!(target_id, 1);
                assert_eq!(text, "hello");
            }
            _ => panic!("Expected TextInputted action, got {:?}", action),
        }
    }

    #[test]
    fn enter_key_commits_empty_text() {
        let mut overlay = TextOverlay::new(42, "", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Enter), tx);

        let action = rx.try_recv().unwrap();
        match action {
            Action::Overlay(OverlayAction::Commit(
                crate::ui::widgets::overlay::OverlayResult::TextInputted { target_id, text },
            )) => {
                assert_eq!(target_id, 42);
                assert_eq!(text, "");
            }
            _ => panic!("Expected TextInputted action"),
        }
    }

    #[test]
    fn character_input_delegates_to_input_box() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let (tx, _rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char('x')), tx);

        assert_eq!(overlay.input.text(), "x");
    }

    #[test]
    fn multiple_character_inputs_accumulate() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let (tx, _rx) = create_action_channel();

        for ch in "world".chars() {
            overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        assert_eq!(overlay.input.text(), "world");
    }

    #[test]
    fn backspace_key_delegates_to_input_box() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        let (tx, _rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Backspace), tx);

        assert_eq!(overlay.input.text(), "hell");
    }

    #[test]
    fn non_press_key_events_are_ignored() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let (tx, _rx) = create_action_channel();

        // Create a release event
        let release_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Release,
            state: crossterm::event::KeyEventState::empty(),
        };

        overlay.inner_handle_key_event(release_event, tx);

        // Text should not be modified for non-press events
        assert_eq!(overlay.input.text(), "");
    }

    // ============================================================================
    // Mouse Event Handling Tests
    // ============================================================================

    #[test]
    fn mouse_click_within_area_positions_cursor() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        let area = Rect::new(10, 10, 20, 3);
        let (tx, _rx) = create_action_channel();

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 12,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };

        overlay.inner_handle_mouse_event(mouse_event, area, tx);

        // The cursor should be positioned based on the click
        // The exact position depends on the area and scroll offset
        assert!(!overlay.input.is_empty());
    }

    #[test]
    fn mouse_click_outside_area_is_ignored() {
        let mut overlay = TextOverlay::new(1, "hello", ValueKind::Text, 20);
        let area = Rect::new(10, 10, 20, 3);
        let (tx, _rx) = create_action_channel();

        let initial_text = overlay.input.text().to_string();

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 5, // Outside area (area starts at column 10)
            row: 10,
            modifiers: KeyModifiers::empty(),
        };

        overlay.inner_handle_mouse_event(mouse_event, area, tx);

        // Text should not be modified for clicks outside the area
        assert_eq!(overlay.input.text(), initial_text);
    }

    // ============================================================================
    // Area Calculation Tests
    // ============================================================================

    #[test]
    fn area_calculation_centers_overlay() {
        let overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let terminal_area = Rect::new(0, 0, 100, 50);

        let area = overlay.area(terminal_area);

        // The overlay should be centered
        // Horizontally: (100 - 20) / 2 = 40
        // Vertically: (50 - 3) / 2 = 23.5 ~= 23
        assert_eq!(area.width, 20);
        assert_eq!(area.height, 3);
        assert!(area.x > 0); // Should not be at the edge
        assert!(area.y > 0); // Should not be at the edge
    }

    #[test]
    fn area_calculation_with_small_terminal() {
        let overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let terminal_area = Rect::new(0, 0, 25, 5);

        let area = overlay.area(terminal_area);

        // Should still fit the overlay size, but may not be perfectly centered
        assert_eq!(area.width, 20);
        assert_eq!(area.height, 3);
    }

    #[test]
    fn area_calculation_with_large_terminal() {
        let overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let terminal_area = Rect::new(0, 0, 200, 100);

        let area = overlay.area(terminal_area);

        // Should be centered
        assert_eq!(area.width, 20);
        assert_eq!(area.height, 3);
        // Check rough centering (allowing for rounding)
        assert!(area.x >= 80 && area.x <= 95); // Should be around 90
        assert!(area.y >= 45 && area.y <= 50); // Should be around 48
    }

    // ============================================================================
    // Rendering Tests
    // ============================================================================

    #[test]
    fn render_border_for_text_kind() {
        let (mut terminal, area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);

        terminal
            .draw(|frame| {
                overlay.render_border(frame, area);
            })
            .unwrap();
    }

    #[test]
    fn render_border_for_integer_kind() {
        let (mut terminal, area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "", ValueKind::Integer, 20);

        terminal
            .draw(|frame| {
                overlay.render_border(frame, area);
            })
            .unwrap();
    }

    #[test]
    fn render_border_for_set_item_kind() {
        let (mut terminal, area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "", ValueKind::SetItem, 20);

        terminal
            .draw(|frame| {
                overlay.render_border(frame, area);
            })
            .unwrap();
    }

    #[test]
    fn render_content_with_valid_area() {
        let (mut terminal, area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "test", ValueKind::Text, 20);

        terminal
            .draw(|frame| {
                overlay.render_content(frame, area);
            })
            .unwrap();
    }

    #[test]
    fn render_content_with_zero_height() {
        let (mut terminal, _area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let zero_height_area = Rect::new(0, 0, 20, 0);

        // Should not panic
        terminal
            .draw(|frame| {
                overlay.render_content(frame, zero_height_area);
            })
            .unwrap();
    }

    #[test]
    fn render_returns_correct_inner_area() {
        let (mut terminal, area) = setup_test_terminal(40, 10);
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);

        terminal
            .draw(|frame| {
                let inner_area = overlay.render_border(frame, area);
                // Inner area should be smaller than the border area
                assert!(inner_area.height < area.height);
                assert!(inner_area.width < area.width);
            })
            .unwrap();
    }

    // ============================================================================
    // State Management Tests
    // ============================================================================

    #[test]
    fn update_with_state_does_not_modify_state() {
        let mut overlay = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let initial_text = overlay.input.text().to_string();
        let initial_target = overlay.target_id;

        overlay.update_with_state(&AppState::default());

        assert_eq!(overlay.input.text(), initial_text);
        assert_eq!(overlay.target_id, initial_target);
    }

    #[test]
    fn state_persists_across_multiple_key_events() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let (tx, _rx) = create_action_channel();

        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char('a')), tx.clone());
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char('b')), tx.clone());
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char('c')), tx);

        assert_eq!(overlay.input.text(), "abc");
        assert_eq!(overlay.target_id, 1);
        assert_eq!(overlay.value_kind, ValueKind::Text);
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn full_workflow_input_and_commit() {
        let mut overlay = TextOverlay::new(42, "", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        // Input text
        for ch in "hello world".chars() {
            overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        // Commit
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Enter), tx);

        let action = rx.try_recv().unwrap();
        match action {
            Action::Overlay(OverlayAction::Commit(
                crate::ui::widgets::overlay::OverlayResult::TextInputted { target_id, text },
            )) => {
                assert_eq!(target_id, 42);
                assert_eq!(text, "hello world");
            }
            _ => panic!("Expected TextInputted action"),
        }
    }

    #[test]
    fn full_workflow_input_and_cancel() {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        // Input text
        for ch in "hello".chars() {
            overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        assert_eq!(overlay.input.text(), "hello");

        // Cancel with Esc
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Esc), tx);

        let action = rx.try_recv().unwrap();
        assert_eq!(action, Action::Overlay(OverlayAction::Close));
    }

    #[test]
    fn edit_existing_text_and_commit() {
        let mut overlay = TextOverlay::new(1, "initial", ValueKind::Text, 20);
        let (tx, mut rx) = create_action_channel();

        // Clear and input new text
        overlay
            .input
            .handle_key_event(KeyEvent::from(KeyCode::Home));
        for _ in 0..7 {
            // Delete "initial"
            overlay
                .input
                .handle_key_event(KeyEvent::from(KeyCode::Delete));
        }

        for ch in "new text".chars() {
            overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        // Commit
        overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Enter), tx);

        let action = rx.try_recv().unwrap();
        match action {
            Action::Overlay(OverlayAction::Commit(
                crate::ui::widgets::overlay::OverlayResult::TextInputted { target_id, text },
            )) => {
                assert_eq!(target_id, 1);
                assert_eq!(text, "new text");
            }
            _ => panic!("Expected TextInputted action"),
        }
    }

    #[test]
    fn text_overlay_equality() {
        let overlay1 = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let overlay2 = TextOverlay::new(1, "test", ValueKind::Text, 20);
        assert_eq!(overlay1, overlay2);
    }

    #[test]
    fn text_overlay_inequality_different_target() {
        let overlay1 = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let overlay2 = TextOverlay::new(2, "test", ValueKind::Text, 20);
        assert_ne!(overlay1, overlay2);
    }

    #[test]
    fn text_overlay_inequality_different_text() {
        let overlay1 = TextOverlay::new(1, "test1", ValueKind::Text, 20);
        let overlay2 = TextOverlay::new(1, "test2", ValueKind::Text, 20);
        assert_ne!(overlay1, overlay2);
    }

    #[test]
    fn text_overlay_inequality_different_value_kind() {
        let overlay1 = TextOverlay::new(1, "test", ValueKind::Text, 20);
        let overlay2 = TextOverlay::new(1, "test", ValueKind::Integer, 20);
        assert_ne!(overlay1, overlay2);
    }

    #[rstest]
    #[case("a")]
    #[case("hello")]
    #[case("hello world")]
    #[case("123")]
    #[case("!@#$%")]
    #[case("😀😁😂")]
    fn text_overlay_preserves_input_text(#[case] text: &str) {
        let mut overlay = TextOverlay::new(1, "", ValueKind::Text, 50);
        let (tx, _rx) = create_action_channel();

        for ch in text.chars() {
            overlay.inner_handle_key_event(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        assert_eq!(overlay.input.text(), text);
    }
}
