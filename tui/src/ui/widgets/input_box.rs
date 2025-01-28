//! Implementation of a search bar

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use ratatui::{
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Paragraph},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::Action,
    ui::{
        components::{Component, ComponentRender},
        AppState,
    },
};

#[derive(Debug)]
pub struct InputBox {
    /// Current value of the input box
    text: String,
    /// Position of cursor in the editor area.
    cursor_position: usize,
}

impl InputBox {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, new_text: &str) {
        self.text = String::from(new_text);
        self.cursor_position = self.text.len();
    }

    pub fn reset(&mut self) {
        self.cursor_position = 0;
        self.text.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.cursor_position.saturating_sub(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.cursor_position.saturating_add(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        self.text.insert(self.cursor_position, new_char);

        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.cursor_position != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.cursor_position;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.text.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.text.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.text = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.text.len())
    }
}

impl Component for InputBox {
    fn new(_state: &AppState, _action_tx: UnboundedSender<Action>) -> Self {
        Self {
            //
            text: String::new(),
            cursor_position: 0,
        }
    }

    fn move_with_state(self, _state: &AppState) -> Self
    where
        Self: Sized,
    {
        self
    }

    fn name(&self) -> &'static str {
        "Input Box"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char(to_insert) => {
                self.enter_char(to_insert);
            }
            KeyCode::Backspace => {
                self.delete_char();
            }
            KeyCode::Left => {
                self.move_cursor_left();
            }
            KeyCode::Right => {
                self.move_cursor_right();
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.text.len();
            }
            _ => {}
        }
    }

    /// Handle mouse events
    ///
    /// moves the cursor to the clicked position
    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        if !area.contains(mouse_position) {
            return;
        }

        if kind == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            // NOTE: this assumes that the border is 1 character wide, which may not necessarily be true
            let mouse_x = mouse_position.x.saturating_sub(area.x + 1) as usize;

            self.cursor_position = self.clamp_cursor(mouse_x);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderProps<'a> {
    pub border: Block<'a>,
    pub area: Rect,
    pub text_color: ratatui::style::Color,
    pub show_cursor: bool,
}

impl<'a> ComponentRender<RenderProps<'a>> for InputBox {
    fn render_border(&self, frame: &mut Frame, props: RenderProps<'a>) -> RenderProps<'a> {
        let view_area = props.border.inner(props.area);
        frame.render_widget(&props.border, props.area);
        RenderProps {
            area: view_area,
            ..props
        }
    }

    fn render_content(&self, frame: &mut Frame, props: RenderProps<'a>) {
        let input = Paragraph::new(self.text.as_str()).style(Style::default().fg(props.text_color));
        frame.render_widget(input, props.area);

        // Cursor is hidden by default, so we need to make it visible if the input box is selected
        if props.show_cursor {
            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[allow(clippy::cast_possible_truncation)]
            frame.set_cursor_position(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                (props.area.x + self.cursor_position as u16, props.area.y),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::setup_test_terminal;

    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use rstest::rstest;

    #[test]
    fn test_input_box() {
        let mut input_box = InputBox {
            text: String::new(),
            cursor_position: 0,
        };

        input_box.enter_char('a');
        assert_eq!(input_box.text, "a");
        assert_eq!(input_box.cursor_position, 1);

        input_box.enter_char('b');
        assert_eq!(input_box.text, "ab");
        assert_eq!(input_box.cursor_position, 2);

        input_box.enter_char('c');
        assert_eq!(input_box.text, "abc");
        assert_eq!(input_box.cursor_position, 3);

        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 2);

        input_box.delete_char();
        assert_eq!(input_box.text, "ac");
        assert_eq!(input_box.cursor_position, 1);

        input_box.enter_char('d');
        assert_eq!(input_box.text, "adc");
        assert_eq!(input_box.cursor_position, 2);

        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 3);

        input_box.reset();
        assert_eq!(input_box.text, "");
        assert_eq!(input_box.cursor_position, 0);

        input_box.delete_char();
        assert_eq!(input_box.text, "");
        assert_eq!(input_box.cursor_position, 0);

        input_box.delete_char();
        assert_eq!(input_box.text, "");
        assert_eq!(input_box.cursor_position, 0);
    }

    #[test]
    fn test_input_box_clamp_cursor() {
        let input_box = InputBox {
            text: String::new(),
            cursor_position: 0,
        };

        assert_eq!(input_box.clamp_cursor(0), 0);
        assert_eq!(input_box.clamp_cursor(1), 0);

        let input_box = InputBox {
            text: "abc".to_string(),
            cursor_position: 3,
        };

        assert_eq!(input_box.clamp_cursor(3), 3);
        assert_eq!(input_box.clamp_cursor(4), 3);
    }

    #[test]
    fn test_input_box_is_empty() {
        let input_box = InputBox {
            text: String::new(),
            cursor_position: 0,
        };

        assert!(input_box.is_empty());

        let input_box = InputBox {
            text: "abc".to_string(),
            cursor_position: 3,
        };

        assert!(!input_box.is_empty());
    }

    #[test]
    fn test_input_box_text() {
        let input_box = InputBox {
            text: "abc".to_string(),
            cursor_position: 3,
        };

        assert_eq!(input_box.text(), "abc");
    }

    #[rstest]
    fn test_input_box_render(
        #[values(10, 20)] width: u16,
        #[values(1, 2, 3, 4, 5, 6)] height: u16,
        #[values(true, false)] show_cursor: bool,
    ) -> Result<()> {
        use ratatui::{buffer::Buffer, text::Line};

        let (mut terminal, _) = setup_test_terminal(width, height);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let mut input_box = InputBox::new(&AppState::default(), action_tx);
        input_box.set_text("Hello, World!");
        let props = RenderProps {
            border: Block::bordered(),
            area: Rect::new(0, 0, width, height),
            text_color: Color::Reset,
            show_cursor,
        };
        let buffer = terminal
            .draw(|frame| input_box.render(frame, props))?
            .buffer
            .clone();

        let line_top = Line::raw(String::from("┌") + &"─".repeat((width - 2).into()) + "┐");
        let line_text = if width > 15 {
            Line::raw(String::from("│Hello, World!") + &" ".repeat((width - 15).into()) + "│")
        } else {
            Line::raw(
                "│Hello, World!"
                    .chars()
                    .take((width - 1).into())
                    .collect::<String>()
                    + "│",
            )
        };
        let line_empty = Line::raw(String::from("│") + &" ".repeat((width - 2).into()) + "│");
        let line_bottom = Line::raw(String::from("└") + &"─".repeat((width - 2).into()) + "┘");

        let expected = Buffer::with_lines(match height {
            0 => unreachable!(),
            1 => vec![line_top].into_iter(),
            2 => vec![line_top, line_bottom].into_iter(),
            3 => vec![line_top, line_text, line_bottom].into_iter(),
            other => vec![line_top, line_text]
                .into_iter()
                .chain(
                    std::iter::repeat(line_empty)
                        .take((other - 3).into())
                        .chain(std::iter::once(line_bottom)),
                )
                .collect::<Vec<_>>()
                .into_iter(),
        });

        assert_eq!(buffer, expected);

        Ok(())
    }
}
