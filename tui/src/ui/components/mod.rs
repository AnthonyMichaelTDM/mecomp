pub mod content_view;
pub mod control_panel;
pub mod queuebar;
pub mod sidebar;

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::action::Action;

use super::AppState;

#[derive(Debug, Clone, Copy)]
pub struct RenderProps {
    pub area: Rect,
    pub is_focused: bool,
}

pub trait Component {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized;
    #[must_use]
    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized;

    fn name(&self) -> &str;

    fn handle_key_event(&mut self, key: KeyEvent);

    fn handle_mouse_event(&mut self, _mouse: MouseEvent, _area: Rect);
}

pub trait ComponentRender<Props> {
    /// Render the border of the view, and return the props updated with the remaining area for the view.
    fn render_border(&self, frame: &mut Frame<'_>, props: Props) -> Props;

    /// Render the view's content.
    fn render_content(&self, frame: &mut Frame<'_>, props: Props);

    /// Render the view (border and content).
    fn render(&self, frame: &mut Frame<'_>, props: Props) {
        let props = self.render_border(frame, props);
        self.render_content(frame, props);
    }
}
