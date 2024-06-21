use crossterm::event::KeyEvent;
use ratatui::{prelude::Rect, text::Line};

use crate::ui::components::ComponentRender;

use super::Popup;

pub struct Notification<'a>(pub Line<'a>);

impl<'a> ComponentRender<Rect> for Notification<'a> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
        frame.render_widget::<Line>(self.0.clone(), area);
    }
}

impl<'a> Popup for Notification<'a> {
    fn title(&self) -> Line {
        Line::from("Dummy Popup")
    }

    fn instructions(&self) -> Line {
        Line::from("Press ESC to close")
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        // put in the top left corner, give enough width to display the text (cap at 50% of the terminal width)
        let max_width = terminal_area.width / 2;
        let width = std::cmp::min(
            u16::try_from(self.0.width() + 2 + 2).unwrap_or(max_width),
            max_width,
        );
        let height = 1 + 2 + 2;
        Rect::new(0, 0, width, height)
    }

    fn inner_handle_key_event(&mut self, _key: KeyEvent) {}
}
