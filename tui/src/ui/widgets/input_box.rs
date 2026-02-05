//! Implementation of a search bar input box widget

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Offset, Position, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthChar;

/// State for the input box widget containing all mutable data
#[derive(Debug, Default)]
pub struct InputBoxState {
    /// Current value of the input box
    text: String,
    /// Index of cursor in the text.
    /// This is in *characters*, not bytes.
    cursor_position: usize,
    /// length of the text in characters
    text_length: usize,
    /// prefix sum array of the text width in columns
    ps_columns: util::PrefixSumVec,
    /// The offset of where the cursor is in the currently displayed area
    cursor_offset: u16,
    /// Horizontal scroll offset in columns (maintains smooth scrolling)
    horizontal_scroll: u16,
}

impl InputBoxState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_text(&mut self, new_text: &str) {
        self.text = String::from(new_text);
        self.text_length = self.text.chars().count();
        self.cursor_position = self.text_length;
        self.ps_columns.clear();
        for c in self.text.chars() {
            self.ps_columns
                .push(UnicodeWidthChar::width(c).unwrap_or_default());
        }
        // Reset scroll when setting new text
        self.horizontal_scroll = 0;
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
        self.text_length = 0;
        self.ps_columns.clear();
        self.horizontal_scroll = 0;
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    const fn move_cursor_left(&mut self) {
        let mut min = self.cursor_position.saturating_sub(1);
        if min > self.text_length {
            min = self.text_length;
        }
        self.cursor_position = min;
    }

    const fn move_cursor_right(&mut self) {
        let mut min = self.cursor_position.saturating_add(1);
        if min > self.text_length {
            min = self.text_length;
        }
        self.cursor_position = min;
    }

    const fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    const fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.text_length;
    }

    const fn update_cursor_offset(&mut self, new_offset: u16) {
        self.cursor_offset = new_offset;
    }

    #[must_use]
    pub const fn cursor_offset(&self) -> Offset {
        Offset::new(self.cursor_offset as i32, 0)
    }

    fn enter_char(&mut self, new_char: char) {
        // we need to convert the cursor position (which is in characters) to the byte index
        // of the cursor position in the string
        let cursor_byte_index = self
            .text
            .chars()
            .take(self.cursor_position)
            .map(char::len_utf8)
            .sum();

        self.text.insert(cursor_byte_index, new_char);
        self.text_length += 1;
        self.ps_columns.insert(
            self.cursor_position,
            UnicodeWidthChar::width(new_char).unwrap_or_default(),
        );

        self.move_cursor_right();
    }

    // Delete the character before the cursor (backspace)
    fn delete_char(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        // Method "remove" is not used on the saved text for deleting the selected char.
        // Reason: Using remove on String works on bytes instead of the chars.
        // Using remove would require special care because of char boundaries.
        let mut chars = self.text.chars();

        // Getting all characters before the selected character.
        let mut new = chars
            .by_ref()
            .take(self.cursor_position - 1)
            .collect::<String>();
        // the character being removed
        chars.next();
        // Getting all characters after selected character.
        new.extend(chars);

        self.text = new;
        self.text_length = self.text_length.saturating_sub(1);
        self.ps_columns.remove(self.cursor_position - 1);
        self.move_cursor_left();
    }

    // delete the character under the cursor (delete)
    fn delete_next_char(&mut self) {
        // same procedure as with `self.delete_char()`, but we don't need to
        // decrement the cursor position
        let mut chars = self.text.chars();
        let mut new = chars
            .by_ref()
            .take(self.cursor_position)
            .collect::<String>();
        chars.next();
        new.extend(chars);

        self.text = new;
        self.text_length = self.text_length.saturating_sub(1);
        self.ps_columns.remove(self.cursor_position);
    }

    /// Update the horizontal scroll offset to keep the cursor visible
    ///
    /// Only scrolls when the cursor moves outside the visible area.
    /// This maintains scroll position when cursor is visible, creating smooth scrolling.
    const fn update_scroll(&mut self, view_width: u16) {
        let cursor_column = self.ps_columns.get(self.cursor_position);
        let scroll = self.horizontal_scroll as usize;
        let view_end = scroll + view_width as usize;

        #[allow(clippy::cast_possible_truncation)]
        if cursor_column < scroll {
            // Cursor moved left past visible area - scroll left to make it visible
            self.horizontal_scroll = cursor_column as u16;
        } else if cursor_column > view_end {
            // Cursor moved right past visible area - scroll right to make it visible
            // Note: cursor_column == view_end is OK (cursor at right edge)
            self.horizontal_scroll = (cursor_column.saturating_sub(view_width as usize)) as u16;
        }
        // else: cursor is within visible area, keep current scroll
    }

    /// Calculate the horizontal scroll offset based on the current cursor position and view width
    const fn calculate_horizontal_scroll(&self, _view_width: u16) -> u16 {
        self.horizontal_scroll
    }

    /// Convert a column position to a character index
    ///
    /// Finds the character index where the cumulative width is closest to the target column.
    /// For wide characters, snaps to the nearest character boundary.
    const fn column_to_char_index(&self, column: usize) -> usize {
        // Binary search to find the character index
        // We want to find the largest index where ps_columns.get(index) <= column
        let mut left = 0;
        let mut right = self.text_length;

        while left < right {
            let mid = (left + right).div_ceil(2);
            if self.ps_columns.get(mid) <= column {
                left = mid;
            } else {
                right = mid - 1;
            }
        }

        left
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
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
            KeyCode::Delete => {
                self.delete_next_char();
            }
            KeyCode::Left => {
                self.move_cursor_left();
            }
            KeyCode::Right => {
                self.move_cursor_right();
            }
            KeyCode::Home => {
                self.move_cursor_to_start();
            }
            KeyCode::End => {
                self.move_cursor_to_end();
            }
            _ => {}
        }
    }

    /// Handle mouse events
    ///
    /// moves the cursor to the clicked position
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        if !area.contains(mouse_position) {
            return;
        }

        if kind == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            // NOTE: this assumes that the border is 1 character wide, which may not necessarily be true
            let mouse_x = mouse_position.x.saturating_sub(area.x + 1);

            // Calculate the view width (accounting for border)
            let view_width = area.width.saturating_sub(2);

            // Calculate current horizontal scroll
            let horizontal_scroll = self.calculate_horizontal_scroll(view_width);

            // Add scroll offset to get the actual column position in the text
            let actual_column = (mouse_x + horizontal_scroll) as usize;

            // Convert column position to character index
            self.cursor_position = self.column_to_char_index(actual_column);
        }
    }
}

/// Input box widget for text input
///
/// This is a stateful widget - use with `InputBoxState` to maintain the input state.
#[derive(Debug, Clone)]
pub struct InputBox<'a> {
    border: Option<Block<'a>>,
    text_color: Color,
}

impl<'a> InputBox<'a> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            border: None,
            text_color: Color::Reset,
        }
    }

    #[must_use]
    pub fn border(mut self, border: Block<'a>) -> Self {
        self.border.replace(border);
        self
    }

    #[must_use]
    pub const fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }
}

impl Default for InputBox<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for InputBox<'_> {
    type State = InputBoxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Get the inner area inside a possible border
        let inner_area = self.border.map_or(area, |border| {
            let inner = border.inner(area);
            border.render(area, buf);
            inner
        });

        // Update scroll to keep cursor visible
        state.update_scroll(inner_area.width);

        let cursor_column = state.ps_columns.get(state.cursor_position);
        let horizontal_scroll = state.calculate_horizontal_scroll(inner_area.width);

        #[allow(clippy::cast_possible_truncation)]
        state.update_cursor_offset(cursor_column as u16 - horizontal_scroll);
        let input = Paragraph::new(state.text.as_str())
            .style(Style::default().fg(self.text_color))
            .scroll((0, horizontal_scroll));

        input.render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{assert_buffer_eq, setup_test_terminal};

    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[test]
    fn test_enter_delete() {
        let mut input_box = InputBoxState::default();

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

        input_box.clear();
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
    fn test_enter_delete_non_ascii_char() {
        let mut input_box = InputBoxState::default();

        input_box.enter_char('a');
        assert_eq!(input_box.text, "a");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 1);
        assert_eq!(input_box.ps_columns.last(), 1);

        input_box.enter_char('m');
        assert_eq!(input_box.text, "am");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.ps_columns.last(), 2);

        input_box.enter_char('é');
        assert_eq!(input_box.text, "amé");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.ps_columns.last(), 3);

        input_box.enter_char('l');
        assert_eq!(input_box.text, "amél");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.ps_columns.last(), 4);

        input_box.delete_char();
        assert_eq!(input_box.text, "amé");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.ps_columns.last(), 3);

        input_box.delete_char();
        assert_eq!(input_box.text, "am");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.ps_columns.last(), 2);
    }

    #[test]
    fn test_enter_delete_wide_characters() {
        let mut input_box = InputBoxState::default();

        input_box.enter_char('こ');
        assert_eq!(input_box.text, "こ");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 1);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 2);
        assert_eq!(input_box.ps_columns.last(), 2);

        input_box.enter_char('ん');
        assert_eq!(input_box.text, "こん");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 4);
        assert_eq!(input_box.ps_columns.last(), 4);

        input_box.enter_char('に');
        assert_eq!(input_box.text, "こんに");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        assert_eq!(input_box.ps_columns.last(), 6);

        input_box.enter_char('ち');
        assert_eq!(input_box.text, "こんにち");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 8);
        assert_eq!(input_box.ps_columns.last(), 8);

        input_box.enter_char('は');
        assert_eq!(input_box.text, "こんにちは");
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 10);
        assert_eq!(input_box.ps_columns.last(), 10);

        input_box.delete_char();
        assert_eq!(input_box.text, "こんにち");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 8);
        assert_eq!(input_box.ps_columns.last(), 8);

        input_box.delete_char();
        assert_eq!(input_box.text, "こんに");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        assert_eq!(input_box.ps_columns.last(), 6);
    }

    #[test]
    fn test_move_left_right() {
        let mut input_box = InputBoxState::default();

        // string with:
        // - normal ascii
        // - accented character (1 column)
        // - wide character (2 columns)
        // - zero-width character
        input_box.set_text("héこ👨\u{200B}");
        assert_eq!(input_box.text, "héこ👨​");
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        assert_eq!(input_box.ps_columns.last(), 6);

        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);

        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 4);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 2);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 1);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 0);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 0);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 0);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 0);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 1);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 2);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 4);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
    }

    #[test]
    fn test_enter_delete_middle() {
        let mut input_box = InputBoxState::default();

        input_box.set_text("ace");
        assert_eq!(input_box.text, "ace");
        assert_eq!(input_box.cursor_position, 3);

        input_box.move_cursor_left();
        input_box.enter_char('Ü');
        assert_eq!(input_box.text, "acÜe");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 3);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.ps_columns.last(), 4);

        input_box.move_cursor_left();
        input_box.move_cursor_left();
        input_box.enter_char('X');
        assert_eq!(input_box.text, "aXcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 2);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.ps_columns.last(), 5);

        // add two wide characters
        input_box.enter_char('こ');
        input_box.enter_char('い');
        assert_eq!(input_box.text, "aXこいcÜe");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 6);
        assert_eq!(input_box.text_length, 7);
        assert_eq!(input_box.ps_columns.last(), 9);

        input_box.move_cursor_left();
        input_box.delete_char();
        assert_eq!(input_box.text, "aXいcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.ps_columns.get(input_box.cursor_position), 2);
        assert_eq!(input_box.text_length, 6);
        assert_eq!(input_box.ps_columns.last(), 7);

        input_box.delete_next_char();
        assert_eq!(input_box.text, "aXcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.ps_columns.last(), 5);

        input_box.delete_char();
        assert_eq!(input_box.text, "acÜe");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.ps_columns.last(), 4);

        input_box.move_cursor_right();
        input_box.delete_next_char();
        assert_eq!(input_box.text, "ace");
        assert_eq!(input_box.cursor_position, 2);
    }

    #[test]
    fn test_input_box_is_empty() {
        let input_box = InputBoxState::default();
        assert!(input_box.is_empty());

        let mut input_box = InputBoxState::default();
        input_box.set_text("abc");

        assert!(!input_box.is_empty());
    }

    #[test]
    fn test_input_box_text() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("abc");

        assert_eq!(input_box.text(), "abc");
    }

    #[rstest]
    fn test_input_box_render(
        #[values(10, 20)] width: u16,
        #[values(1, 2, 3, 4, 5, 6)] height: u16,
    ) -> Result<()> {
        use ratatui::{buffer::Buffer, text::Line};

        let (mut terminal, _) = setup_test_terminal(width, height);
        let mut state = InputBoxState::default();
        state.set_text("Hello, World!");
        let area = Rect::new(0, 0, width, height);

        let buffer = terminal
            .draw(|frame| {
                frame.render_stateful_widget(
                    InputBox::new().border(Block::bordered()),
                    area,
                    &mut state,
                )
            })?
            .buffer
            .clone();

        let line_top = Line::raw(String::from("┌") + &"─".repeat((width - 2).into()) + "┐");
        let line_text = if width > 15 {
            Line::raw(String::from("│Hello, World!") + &" ".repeat((width - 15).into()) + "│")
        } else {
            Line::raw(
                String::from("│")
                    + &"Hello, World!"
                        .chars()
                        .skip(state.text().len() - (width - 2) as usize)
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
                    std::iter::repeat_n(line_empty, (other - 3).into())
                        .chain(std::iter::once(line_bottom)),
                )
                .collect::<Vec<_>>()
                .into_iter(),
        });

        assert_eq!(buffer, expected);

        Ok(())
    }

    #[rstest]
    #[case::fits("Hello", 10, "Hello     ")]
    #[case::exact_fit("Hello, World!", 13, "Hello, World!")]
    #[case::too_small("Hello, World!", 6, "World!")]
    #[case::too_small_wide("こんにちは世界", 10, "にちは世界")]
    fn test_keeps_cursor_visible_right(
        #[case] new_text: &str,
        #[case] view_width: u16,
        #[case] expected_visible_text: &str,
    ) -> Result<()> {
        use ratatui::{buffer::Buffer, text::Line};

        let (mut terminal, _) = setup_test_terminal(view_width, 1);
        let mut state = InputBoxState::default();
        state.set_text(new_text);

        let area = Rect::new(0, 0, view_width, 1);
        let buffer = terminal
            .draw(|frame| frame.render_stateful_widget(InputBox::new(), area, &mut state))?
            .buffer
            .clone();
        let line = Line::raw(expected_visible_text.to_string());
        let expected = Buffer::with_lines(std::iter::once(line));
        assert_buffer_eq(&buffer, &expected);
        Ok(())
    }

    #[test]
    fn test_column_to_char_index() {
        let mut input_box = InputBoxState::default();

        // Test with ASCII text
        input_box.set_text("Hello, World!");
        assert_eq!(input_box.column_to_char_index(0), 0);
        assert_eq!(input_box.column_to_char_index(1), 1);
        assert_eq!(input_box.column_to_char_index(5), 5);
        assert_eq!(input_box.column_to_char_index(13), 13);
        assert_eq!(input_box.column_to_char_index(100), 13); // Beyond end

        // Test with wide characters
        input_box.set_text("こんにち");
        // Each character is 2 columns wide
        assert_eq!(input_box.column_to_char_index(0), 0);
        assert_eq!(input_box.column_to_char_index(1), 0); // In middle of first char
        assert_eq!(input_box.column_to_char_index(2), 1); // Start of second char
        assert_eq!(input_box.column_to_char_index(3), 1); // In middle of second char
        assert_eq!(input_box.column_to_char_index(4), 2); // Start of third char
        assert_eq!(input_box.column_to_char_index(6), 3); // Start of fourth char
        assert_eq!(input_box.column_to_char_index(8), 4); // Beyond end

        // Test with mixed width characters
        input_box.set_text("aこbに");
        // a=1, こ=2, b=1, に=2 total=6 columns
        assert_eq!(input_box.column_to_char_index(0), 0); // 'a'
        assert_eq!(input_box.column_to_char_index(1), 1); // 'こ' start
        assert_eq!(input_box.column_to_char_index(2), 1); // 'こ' middle
        assert_eq!(input_box.column_to_char_index(3), 2); // 'b'
        assert_eq!(input_box.column_to_char_index(4), 3); // 'に' start
        assert_eq!(input_box.column_to_char_index(5), 3); // 'に' middle
        assert_eq!(input_box.column_to_char_index(6), 4); // Beyond end
    }

    #[test]
    fn test_smooth_scrolling_maintains_position() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("Hello, World! This is long text!"); // 32 chars

        // Text is 32 chars, cursor at end after set_text
        assert_eq!(input_box.text_length, 32);
        assert_eq!(input_box.cursor_position, 32);

        // Update scroll with view width of 10
        input_box.update_scroll(10);
        assert_eq!(input_box.horizontal_scroll, 22); // 32 - 10 = 22

        // Move cursor left a few characters (but still visible)
        for _ in 0..3 {
            input_box.move_cursor_left();
        }
        input_box.update_scroll(10);
        // Scroll should NOT change because cursor is still visible (column 29, view is [22, 32])
        assert_eq!(input_box.cursor_position, 29);
        assert_eq!(input_box.horizontal_scroll, 22);

        // Move cursor even more left (still visible)
        for _ in 0..5 {
            input_box.move_cursor_left();
        }
        input_box.update_scroll(10);
        // Still visible (column 24, view is [22, 32])
        assert_eq!(input_box.cursor_position, 24);
        assert_eq!(input_box.horizontal_scroll, 22);

        // Now move cursor left past the visible area
        for _ in 0..5 {
            input_box.move_cursor_left();
        }
        input_box.update_scroll(10);
        // Should scroll left to make cursor visible at left edge
        assert_eq!(input_box.cursor_position, 19);
        assert_eq!(input_box.horizontal_scroll, 19);
    }

    #[test]
    fn test_smooth_scrolling_right_movement() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("Hello, World! This is long text!"); // 32 chars

        // Start with cursor at beginning (set_text leaves cursor at end, move it back)
        input_box.move_cursor_to_start();
        assert_eq!(input_box.cursor_position, 0);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Move cursor right within visible area
        for _ in 0..5 {
            input_box.move_cursor_right();
        }
        input_box.update_scroll(10);
        // Should not scroll (column 5 is visible in [0, 10])
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Move cursor to exactly at the right edge
        for _ in 0..4 {
            input_box.move_cursor_right();
        }
        input_box.update_scroll(10);
        // Still visible (column 9 is in [0, 10])
        assert_eq!(input_box.cursor_position, 9);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Move cursor to position that equals view_end
        input_box.move_cursor_right();
        input_box.update_scroll(10);
        // Cursor at right edge (column 10 == view_end) - should be visible without scroll
        assert_eq!(input_box.cursor_position, 10);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Move cursor one more past the right edge
        input_box.move_cursor_right();
        input_box.update_scroll(10);
        // Now cursor is not visible (column 11 > view_end 10), should scroll
        assert_eq!(input_box.cursor_position, 11);
        assert_eq!(input_box.horizontal_scroll, 1); // 11 - 10 = 1
    }

    #[test]
    fn test_smooth_scrolling_with_wide_chars() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("こんにちは世界です。"); // 10 chars, 20 columns

        // Cursor at end after set_text (column 20)
        assert_eq!(input_box.cursor_position, 10);
        input_box.update_scroll(10);
        // Cursor at column 20 > view_end 10, need to scroll
        assert_eq!(input_box.horizontal_scroll, 10); // 20 - 10 = 10

        // Move cursor left (still visible with current scroll)
        for _ in 0..3 {
            input_box.move_cursor_left();
        }
        input_box.update_scroll(10);
        // Cursor at position 7, column 14, view is [10, 20], still visible
        assert_eq!(input_box.cursor_position, 7);
        assert_eq!(input_box.horizontal_scroll, 10);

        // Move cursor further left past visible area
        for _ in 0..3 {
            input_box.move_cursor_left();
        }
        input_box.update_scroll(10);
        // Cursor at position 4, column 8 < scroll 10, so scroll to 8
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.horizontal_scroll, 8);
    }

    #[test]
    fn test_smooth_scrolling_edge_cases() {
        let mut input_box = InputBoxState::default();

        // Empty text
        input_box.set_text("");
        input_box.update_scroll(10);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Text shorter than view
        input_box.set_text("Hi");
        input_box.cursor_position = 2;
        input_box.update_scroll(10);
        assert_eq!(input_box.horizontal_scroll, 0);

        // Clear should reset scroll
        input_box.set_text("Long text here");
        input_box.cursor_position = 14;
        input_box.update_scroll(5);
        assert!(input_box.horizontal_scroll > 0);
        input_box.clear();
        assert_eq!(input_box.horizontal_scroll, 0);
    }

    #[test]
    fn test_mouse_click_no_scroll() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("Hello");

        // Click at position 2 (on 'l')
        let mouse_event = MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 3, // area.x=1, border=1, so mouse_x = 3-1-1 = 1, but we want position 2
            row: 1,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let area = Rect::new(1, 1, 10, 1);
        input_box.handle_mouse_event(mouse_event, area);
        assert_eq!(input_box.cursor_position, 1);
    }

    #[test]
    fn test_mouse_click_with_scroll_ascii() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("Hello, World!");

        // Move cursor to end using cursor movement
        for _ in 0..13 {
            input_box.move_cursor_right();
        }
        input_box.update_scroll(10);

        // View width is 10, cursor at column 13, so scroll = 13 - 10 = 3
        assert_eq!(input_box.horizontal_scroll, 3);

        // Click at mouse position 5 (relative to content area)
        // actual_column = 5 + 3 = 8, which should be position 8
        let mouse_event = MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 6, // area.x=0, border=1, mouse_x = 6-0-1 = 5
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 12, 1);
        input_box.handle_mouse_event(mouse_event, area);
        assert_eq!(input_box.cursor_position, 8);
    }

    #[test]
    fn test_mouse_click_with_scroll_wide_chars() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("こんにちは世界"); // 7 chars, 14 columns

        // Move cursor to end using cursor movement
        for _ in 0..7 {
            input_box.move_cursor_right();
        }
        input_box.update_scroll(10);

        // View width is 10, cursor at column 14, so scroll = 14 - 10 = 4
        assert_eq!(input_box.horizontal_scroll, 4);

        // Click at mouse position 6 (relative to content area)
        // actual_column = 6 + 4 = 10
        // Column 10 corresponds to character index 5 (10/2 = 5)
        let mouse_event = MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 7, // area.x=0, border=1, mouse_x = 7-0-1 = 6
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 12, 1);
        input_box.handle_mouse_event(mouse_event, area);
        assert_eq!(input_box.cursor_position, 5);
    }

    #[test]
    fn test_mouse_click_beyond_text_end() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("Hi");

        // Click far beyond the text
        let mouse_event = MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 20,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 30, 1);
        input_box.handle_mouse_event(mouse_event, area);
        assert_eq!(input_box.cursor_position, 2); // Should clamp to text length
    }

    #[test]
    fn test_mouse_click_on_wide_char_boundary() {
        let mut input_box = InputBoxState::default();
        input_box.set_text("aこb");

        // Click on column 2 (middle of 'こ' which spans columns 1-2)
        // Should snap to character index 1
        let mouse_event = MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 3, // area.x=0, border=1, mouse_x = 3-0-1 = 2
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 10, 1);
        input_box.handle_mouse_event(mouse_event, area);
        assert_eq!(input_box.cursor_position, 1);
    }
}

mod util {
    /// A helper struct that maintains a prefix sum array of usize values.
    ///
    /// This is used to efficiently calculate the (column) width of substrings in the input box.
    ///
    /// This implementation optimizes the most common operations (reading and adding/removing from the end) to O(1), at the cost of
    /// making insertions and deletions anywhere else take O(n).
    #[derive(Debug)]
    pub struct PrefixSumVec {
        data: Vec<usize>,
    }
    impl PrefixSumVec {
        pub fn new() -> Self {
            Self { data: vec![0] }
        }

        pub const fn last(&self) -> usize {
            if let [.., last] = self.data.as_slice() {
                *last
            } else {
                unreachable!() // there is always at least one element (0)
            }
        }

        pub fn push(&mut self, value: usize) {
            let last = self.last();
            self.data.push(last + value);
        }

        pub fn insert(&mut self, index: usize, value: usize) {
            // adjust index to account for the leading zero
            let index = index + 1;

            // if trying to insert at the end, just push
            if index >= self.data.len() {
                self.push(value);
                return;
            }

            // adjust all subsequent values, then push to the end.
            // idea is to "add" the value at the index and adjust everything after it in place,
            // without needing to actually perform a full insertion shift
            let mut prev = self.data[index - 1];
            for i in index..self.data.len() {
                let current = self.data[i];
                self.data[i] = prev + value;
                prev = current;
            }
            self.data.push(prev + value);
        }

        pub fn remove(&mut self, index: usize) {
            if self.data.len() <= 1 {
                // nothing to remove
                return;
            }

            // adjust index to account for the leading zero
            let index = index + 1;

            // if trying to remove at the end, just pop instead
            if index >= self.data.len() {
                self.data.pop();
                return;
            }

            // adjust all subsequent values, then pop
            for i in index..self.data.len() - 1 {
                let prev = self.data[i - 1];
                let next = self.data[i + 1];
                self.data[i] = prev + (next - self.data[i]);
            }
            self.data.pop();
        }

        pub const fn get(&self, index: usize) -> usize {
            self.data.as_slice()[index]
        }

        pub fn clear(&mut self) {
            self.data.clear();
            self.data.push(0);
        }
    }
    impl Default for PrefixSumVec {
        fn default() -> Self {
            Self::new()
        }
    }
    #[cfg(test)]
    mod tests {
        use super::PrefixSumVec;
        use pretty_assertions::assert_eq;

        #[test]
        fn test_prefix_sum_vec_basic_operations() {
            let mut psv = PrefixSumVec::new();
            assert_eq!(psv.last(), 0);
            assert_eq!(psv.data, vec![0]);
            assert_eq!(psv.last(), 0);
            psv.remove(0); // removing from empty should do nothing
            assert_eq!(psv.data, vec![0]);
            assert_eq!(psv.last(), 0);
            psv.push(3);
            assert_eq!(psv.data, vec![0, 3]);
            assert_eq!(psv.last(), 3);
            psv.push(5);
            assert_eq!(psv.data, vec![0, 3, 8]);
            assert_eq!(psv.last(), 8);
            psv.insert(1, 2); // insert 2 at index 1
            assert_eq!(psv.data, vec![0, 3, 5, 10]);
            assert_eq!(psv.last(), 10);
            psv.insert(0, 7); // insert 7 at index 0
            assert_eq!(psv.data, vec![0, 7, 10, 12, 17]);
            assert_eq!(psv.last(), 17);
            psv.remove(2); // remove value at index 2
            assert_eq!(psv.data, vec![0, 7, 10, 15]);
            assert_eq!(psv.last(), 15);
            psv.remove(0); // remove value at index 0
            assert_eq!(psv.data, vec![0, 3, 8]);
            psv.remove(1); // remove the last element
            assert_eq!(psv.data, vec![0, 3]);
            assert_eq!(psv.get(0), 0);
            assert_eq!(psv.get(1), 3);
            psv.insert(1, 4); // insert to the end
            assert_eq!(psv.data, vec![0, 3, 7]);
            psv.clear();
            assert_eq!(psv.data, vec![0]);
        }
    }
}
