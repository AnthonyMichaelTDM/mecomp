use crossterm::event::KeyEvent;
use ratatui::{prelude::Rect, text::Line};

use crate::ui::components::ComponentRender;

use super::Popup;

pub struct Notification<T>(pub T)
where
    T: Clone + Send + Sync + Into<Line<'static>>;

impl<T> ComponentRender<Rect> for Notification<T>
where
    T: Clone + Send + Sync + Into<Line<'static>>,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
        frame.render_widget::<Line>(self.0.clone().into(), area);
    }
}

impl<T> Popup for Notification<T>
where
    T: Clone + Send + Sync + Into<Line<'static>>,
{
    fn title(&self) -> Line {
        Line::from("Dummy Popup")
    }

    fn instructions(&self) -> Line {
        Line::from("Press ESC to close")
    }

    fn inner_handle_key_event(&mut self, _key: KeyEvent) {}
}
