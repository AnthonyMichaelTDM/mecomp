pub mod content_view;
pub mod control_panel;
pub mod queuebar;
pub mod sidebar;

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::action::Action;

use super::AppState;

pub struct RenderProps {
    pub area: Rect,
    pub is_focused: bool,
}

pub trait Component {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized;
    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized;

    fn name(&self) -> &str;

    fn handle_key_event(&mut self, key: KeyEvent);
}

pub trait ComponentRender<Props> {
    fn render(&self, frame: &mut Frame, props: Props);
}
