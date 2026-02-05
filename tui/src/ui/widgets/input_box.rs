//! Implementation of a search bar input box component
//!
//! TODO: clicking to move cursor does not account for scrolling

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    widgets::{Block, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    state::action::Action,
    ui::{
        AppState,
        components::{Component, ComponentRender},
    },
};

#[derive(Debug, Default)]
pub struct InputBox {
    /// Current value of the input box
    text: String,
    /// Position of cursor in the text.
    /// This is in *characters*, not bytes.
    cursor_position: usize,
    /// length of the text in characters
    text_length: usize,
    /// Position of the cursor in the editor area
    /// This is in *columns*, not bytes or characters.
    cursor_column: usize,
    /// Width of the text in columns
    /// This is in *columns*, not bytes or characters.
    text_width: usize,
}

impl InputBox {
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_text(&mut self, new_text: &str) {
        self.text = String::from(new_text);
        self.text_length = self.text.chars().count();
        self.cursor_position = self.text_length;
        self.text_width = UnicodeWidthStr::width(new_text);
        self.cursor_column = self.text_width;
    }

    pub fn reset(&mut self) {
        self.cursor_position = 0;
        self.text_length = 0;
        self.cursor_column = 0;
        self.text_width = 0;
        self.text.clear();
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1).min(self.text_length);
        self.cursor_column = self
            .text
            .chars()
            .take(self.cursor_position)
            .map(|c| UnicodeWidthChar::width(c).unwrap_or_default())
            .sum::<usize>()
            .min(self.text_width);
    }

    fn move_cursor_right(&mut self) {
        self.cursor_position = self.cursor_position.saturating_add(1).min(self.text_length);
        self.cursor_column = self
            .text
            .chars()
            .take(self.cursor_position)
            .map(|c| UnicodeWidthChar::width(c).unwrap_or_default())
            .sum::<usize>()
            .min(self.text_width);
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
        self.text_width += UnicodeWidthChar::width(new_char).unwrap_or_default();

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
        let target = chars.next();
        // Getting all characters after selected character.
        new.extend(chars);

        self.text = new;
        self.text_length = self.text_length.saturating_sub(1);
        if let Some(c) = target {
            self.text_width = self
                .text_width
                .saturating_sub(UnicodeWidthChar::width(c).unwrap_or_default());
        }
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
        let target = chars.next();
        new.extend(chars);

        self.text = new;
        self.text_length = self.text_length.saturating_sub(1);
        if let Some(c) = target {
            self.text_width = self
                .text_width
                .saturating_sub(UnicodeWidthChar::width(c).unwrap_or_default());
        }
    }
}

impl Component for InputBox {
    fn new(_state: &AppState, _action_tx: UnboundedSender<Action>) -> Self {
        Self::default()
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
                self.cursor_position = 0;
                self.cursor_column = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.text_length;
                self.cursor_column = self.text_length;
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

            self.cursor_position = mouse_x.min(self.text_length);
            self.cursor_column = self
                .text
                .chars()
                .take(self.cursor_position)
                .map(|c| UnicodeWidthChar::width(c).unwrap_or_default())
                .sum::<usize>()
                .min(self.text_width);
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
    fn render_border(&self, frame: &mut Frame<'_>, props: RenderProps<'a>) -> RenderProps<'a> {
        let view_area = props.border.inner(props.area);
        frame.render_widget(&props.border, props.area);
        RenderProps {
            area: view_area,
            ..props
        }
    }

    fn render_content(&self, frame: &mut Frame<'_>, props: RenderProps<'a>) {
        #[allow(clippy::cast_possible_truncation)]
        let horizontal_scroll = if self.cursor_column > props.area.width as usize {
            self.cursor_column as u16 - props.area.width
        } else {
            0
        };

        let input = Paragraph::new(self.text.as_str())
            .style(Style::default().fg(props.text_color))
            .scroll((0, horizontal_scroll));

        frame.render_widget(input, props.area);

        // Cursor is hidden by default, so we need to make it visible if the input box is selected
        if props.show_cursor {
            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[allow(clippy::cast_possible_truncation)]
            frame.set_cursor_position(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                (
                    props.area.x + self.cursor_column as u16 - horizontal_scroll,
                    props.area.y,
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{assert_buffer_eq, setup_test_terminal};

    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use rstest::rstest;

    #[test]
    fn test_enter_delete() {
        let mut input_box = InputBox::default();

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
    fn test_enter_delete_non_ascii_char() {
        let mut input_box = InputBox::default();

        input_box.enter_char('a');
        assert_eq!(input_box.text, "a");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 1);
        assert_eq!(input_box.cursor_column, 1);
        assert_eq!(input_box.prefix_text_widths.last(), 1);

        input_box.enter_char('m');
        assert_eq!(input_box.text, "am");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.cursor_column, 2);
        assert_eq!(input_box.prefix_text_widths.last(), 2);

        input_box.enter_char('é');
        assert_eq!(input_box.text, "amé");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.cursor_column, 3);
        assert_eq!(input_box.prefix_text_widths.last(), 3);

        input_box.enter_char('l');
        assert_eq!(input_box.text, "amél");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.cursor_column, 4);
        assert_eq!(input_box.prefix_text_widths.last(), 4);

        input_box.delete_char();
        assert_eq!(input_box.text, "amé");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.cursor_column, 3);
        assert_eq!(input_box.prefix_text_widths.last(), 3);

        input_box.delete_char();
        assert_eq!(input_box.text, "am");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.cursor_column, 2);
        assert_eq!(input_box.prefix_text_widths.last(), 2);
    }

    #[test]
    fn test_enter_delete_wide_characters() {
        let mut input_box = InputBox::default();

        input_box.enter_char('こ');
        assert_eq!(input_box.text, "こ");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 1);
        assert_eq!(input_box.cursor_column, 2);
        assert_eq!(input_box.prefix_text_widths.last(), 2);

        input_box.enter_char('ん');
        assert_eq!(input_box.text, "こん");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 2);
        assert_eq!(input_box.cursor_column, 4);
        assert_eq!(input_box.prefix_text_widths.last(), 4);

        input_box.enter_char('に');
        assert_eq!(input_box.text, "こんに");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.cursor_column, 6);
        assert_eq!(input_box.prefix_text_widths.last(), 6);

        input_box.enter_char('ち');
        assert_eq!(input_box.text, "こんにち");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.cursor_column, 8);
        assert_eq!(input_box.prefix_text_widths.last(), 8);

        input_box.enter_char('は');
        assert_eq!(input_box.text, "こんにちは");
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.cursor_column, 10);
        assert_eq!(input_box.prefix_text_widths.last(), 10);

        input_box.delete_char();
        assert_eq!(input_box.text, "こんにち");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.cursor_column, 8);
        assert_eq!(input_box.prefix_text_widths.last(), 8);

        input_box.delete_char();
        assert_eq!(input_box.text, "こんに");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.text_length, 3);
        assert_eq!(input_box.cursor_column, 6);
        assert_eq!(input_box.prefix_text_widths.last(), 6);
    }

    #[test]
    fn test_move_left_right() {
        let mut input_box = InputBox::default();

        // string with:
        // - normal ascii
        // - accented character (1 column)
        // - wide character (2 columns)
        // - zero-width character
        input_box.set_text("héこ👨\u{200B}");
        assert_eq!(input_box.text, "héこ👨​");
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.cursor_column, 6);
        assert_eq!(input_box.prefix_text_widths.last(), 6);

        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.cursor_column, 6);

        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.cursor_column, 4);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.cursor_column, 2);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.cursor_column, 1);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 0);
        assert_eq!(input_box.cursor_column, 0);
        input_box.move_cursor_left();
        assert_eq!(input_box.cursor_position, 0);
        assert_eq!(input_box.cursor_column, 0);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.cursor_column, 1);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.cursor_column, 2);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.cursor_column, 4);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.cursor_column, 6);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.cursor_column, 6);
        input_box.move_cursor_right();
        assert_eq!(input_box.cursor_position, 5);
        assert_eq!(input_box.cursor_column, 6);
    }

    #[test]
    fn test_enter_delete_middle() {
        let mut input_box = InputBox::default();

        input_box.set_text("ace");
        assert_eq!(input_box.text, "ace");
        assert_eq!(input_box.cursor_position, 3);

        input_box.move_cursor_left();
        input_box.enter_char('Ü');
        assert_eq!(input_box.text, "acÜe");
        assert_eq!(input_box.cursor_position, 3);
        assert_eq!(input_box.cursor_column, 3);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.prefix_text_widths.last(), 4);

        input_box.move_cursor_left();
        input_box.move_cursor_left();
        input_box.enter_char('X');
        assert_eq!(input_box.text, "aXcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.cursor_column, 2);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.prefix_text_widths.last(), 5);

        // add two wide characters
        input_box.enter_char('こ');
        input_box.enter_char('い');
        assert_eq!(input_box.text, "aXこいcÜe");
        assert_eq!(input_box.cursor_position, 4);
        assert_eq!(input_box.cursor_column, 6);
        assert_eq!(input_box.text_length, 7);
        assert_eq!(input_box.prefix_text_widths.last(), 9);

        input_box.move_cursor_left();
        input_box.delete_char();
        assert_eq!(input_box.text, "aXいcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.cursor_column, 2);
        assert_eq!(input_box.text_length, 6);
        assert_eq!(input_box.prefix_text_widths.last(), 7);

        input_box.delete_next_char();
        assert_eq!(input_box.text, "aXcÜe");
        assert_eq!(input_box.cursor_position, 2);
        assert_eq!(input_box.text_length, 5);
        assert_eq!(input_box.prefix_text_widths.last(), 5);

        input_box.delete_char();
        assert_eq!(input_box.text, "acÜe");
        assert_eq!(input_box.cursor_position, 1);
        assert_eq!(input_box.text_length, 4);
        assert_eq!(input_box.prefix_text_widths.last(), 4);

        input_box.move_cursor_right();
        input_box.delete_next_char();
        assert_eq!(input_box.text, "ace");
        assert_eq!(input_box.cursor_position, 2);
    }

    #[test]
    fn test_input_box_is_empty() {
        let input_box = InputBox::default();
        assert!(input_box.is_empty());

        let mut input_box = InputBox::default();
        input_box.set_text("abc");

        assert!(!input_box.is_empty());
    }

    #[test]
    fn test_input_box_text() {
        let mut input_box = InputBox::default();
        input_box.set_text("abc");

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
                String::from("│")
                    + &"Hello, World!"
                        .chars()
                        .skip(input_box.text.len() - (width - 2) as usize)
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
        let mut input_box = InputBox::default();
        input_box.set_text(new_text);

        let props = RenderProps {
            border: Block::new(),
            area: Rect::new(0, 0, view_width, 1),
            text_color: Color::Reset,
            show_cursor: true,
        };
        let buffer = terminal
            .draw(|frame| input_box.render(frame, props))?
            .buffer
            .clone();
        let line = Line::raw(expected_visible_text.to_string());
        let expected = Buffer::with_lines(std::iter::once(line));
        assert_buffer_eq(&buffer, &expected);
        Ok(())
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
