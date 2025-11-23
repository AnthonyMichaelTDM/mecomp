use crossterm::event::{KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::Rect,
    text::{Line, Text},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, PopupAction},
    ui::components::ComponentRender,
};

use super::Popup;

#[derive(Debug)]
pub struct Notification<'a> {
    pub line: Text<'a>,
    pub action_tx: UnboundedSender<Action>,
}

impl<'a> Notification<'a> {
    #[must_use]
    pub const fn new(line: Text<'a>, action_tx: UnboundedSender<Action>) -> Self {
        Self { line, action_tx }
    }
}

impl ComponentRender<Rect> for Notification<'_> {
    fn render_border(&self, frame: &mut ratatui::Frame<'_>, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget::<Text<'_>>(self.line.clone(), area);
    }
}

impl Popup for Notification<'_> {
    fn title(&self) -> Line<'static> {
        Line::raw("Notification")
    }

    fn instructions(&self) -> Line<'static> {
        Line::raw("Press ESC to close")
    }

    fn update_with_state(&mut self, _: &crate::ui::AppState) {}

    fn area(&self, terminal_area: Rect) -> Rect {
        // put in the top left corner, give enough width/height to display the text, and add 2 for the border
        let width = u16::try_from(
            self.line
                .width()
                .max(self.instructions().width())
                .max(self.title().width())
                + 2,
        )
        .unwrap_or(terminal_area.width)
        .min(terminal_area.width);
        let height = u16::try_from(self.line.height() + 2)
            .unwrap_or(terminal_area.height)
            .min(terminal_area.height);
        Rect::new(0, 0, width, height)
    }

    fn inner_handle_key_event(&mut self, _key: KeyEvent) {}

    fn inner_handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent, _: Rect) {
        // Close the popup when the mouse is clicked
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            self.action_tx.send(Action::Popup(PopupAction::Close)).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_terminal;
    use anyhow::Result;
    use crossterm::event::{KeyModifiers, MouseEvent};
    use pretty_assertions::assert_eq;
    use ratatui::{
        buffer::Buffer,
        style::{Color, Style},
        text::Span,
    };
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn test_notification_area() {
        let (_, area) = setup_test_terminal(100, 100);
        let area = Notification::new(Text::from("Hello, World!"), unbounded_channel().0).area(area);
        assert_eq!(area, Rect::new(0, 0, 20, 3));
    }

    #[test]
    fn test_notification_render() -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(20, 3);
        let notification = Notification::new(Text::from("Hello, World!"), unbounded_channel().0);
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
        let notification = Notification::new(Text::from("Hello, World!"), unbounded_channel().0);
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
    fn test_notification_render_multiline() -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(20, 5);
        let notification = Notification::new(Text::from("Hello,\nWorld!"), unbounded_channel().0);
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

    #[test]
    fn test_click_to_close() {
        let (mut terminal, area) = setup_test_terminal(20, 3);
        let (action_tx, mut action_rx) = unbounded_channel();
        let mut notification = Notification::new(Text::from("Hello, World!"), action_tx.clone());
        terminal
            .draw(|frame| notification.render_popup(frame))
            .unwrap();
        notification.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::empty(),
            },
            area,
            action_tx,
        );
        assert_eq!(
            action_rx.try_recv().unwrap(),
            Action::Popup(PopupAction::Close)
        );
    }
}
