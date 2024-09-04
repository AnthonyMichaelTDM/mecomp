use crossterm::event::KeyEvent;
use ratatui::{
    prelude::Rect,
    text::{Line, Text},
};

use crate::ui::components::ComponentRender;

use super::Popup;

pub struct Notification<'a>(pub Text<'a>);

impl<'a> ComponentRender<Rect> for Notification<'a> {
    fn render_border(&self, frame: &mut ratatui::Frame, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&self, frame: &mut ratatui::Frame, area: Rect) {
        frame.render_widget::<Text>(self.0.clone(), area);
    }
}

impl<'a> Popup for Notification<'a> {
    fn title(&self) -> Line {
        Line::raw("Notification")
    }

    fn instructions(&self) -> Line {
        Line::raw("Press ESC to close")
    }

    fn update_with_state(&mut self, _: &crate::ui::AppState) {}

    fn area(&self, terminal_area: Rect) -> Rect {
        // put in the top left corner, give enough width/height to display the text, and add 2 for the border
        let width = u16::try_from(
            self.0
                .width()
                .max(self.instructions().width())
                .max(self.title().width())
                + 2,
        )
        .unwrap_or(terminal_area.width)
        .min(terminal_area.width);
        let height = u16::try_from(self.0.height() + 2)
            .unwrap_or(terminal_area.height)
            .min(terminal_area.height);
        Rect::new(0, 0, width, height)
    }

    fn inner_handle_key_event(&mut self, _key: KeyEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_terminal;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use ratatui::{
        buffer::Buffer,
        style::{Color, Style},
        text::Span,
    };

    #[test]
    fn test_notification_area() -> Result<()> {
        let (_, area) = setup_test_terminal(100, 100);
        let area = Notification(Text::from("Hello, World!")).area(area);
        assert_eq!(area, Rect::new(0, 0, 20, 3));
        Ok(())
    }

    #[test]
    fn test_notification_render() -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(20, 3);
        let notification = Notification(Text::from("Hello, World!"));
        let buffer = terminal
            .draw(|frame| notification.render_popup(frame))?
            .buffer
            .clone();
        let style = Style::reset().fg(Color::Rgb(3, 169, 244));
        let expected = Buffer::with_lines([
            Line::styled("┌Notification──────┐", style),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("Hello, World!     "),
                Span::styled("│", style),
            ]),
            Line::styled("└Press ESC to close┘", style),
        ]);
        assert_eq!(buffer, expected);
        Ok(())
    }

    #[test]
    fn test_notification_render_small_terminal() -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(18, 2);
        let notification = Notification(Text::from("Hello, World!"));
        let buffer = terminal
            .draw(|frame| notification.render_popup(frame))?
            .buffer
            .clone();
        let style = Style::reset().fg(Color::Rgb(3, 169, 244));
        let expected = Buffer::with_lines([
            Line::styled("┌Notification────┐", style),
            Line::styled("└Press ESC to clo┘", style),
        ]);
        assert_eq!(buffer, expected);
        Ok(())
    }

    #[test]
    fn test_nofitication_render_multiline() -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(20, 5);
        let notification = Notification(Text::from("Hello,\nWorld!"));
        let buffer = terminal
            .draw(|frame| notification.render_popup(frame))?
            .buffer
            .clone();
        let style = Style::reset().fg(Color::Rgb(3, 169, 244));
        let expected = Buffer::with_lines([
            Line::styled("┌Notification──────┐", style),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("Hello,            "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("World!            "),
                Span::styled("│", style),
            ]),
            Line::styled("└Press ESC to close┘", style),
            Line::raw("                    "),
        ]);
        assert_eq!(buffer, expected);
        Ok(())
    }
}
