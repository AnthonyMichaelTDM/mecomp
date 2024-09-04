pub mod notification;
pub mod playlist;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Clear},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, PopupAction},
    ui::{colors::POPUP_BORDER, components::ComponentRender, AppState},
};

pub trait Popup: for<'a> ComponentRender<Rect> + Send + Sync {
    fn title(&self) -> Line;
    fn instructions(&self) -> Line;
    /// The area needed for the popup to render.
    fn area(&self, terminal_area: Rect) -> Rect;

    /// override this method to change the border color of the popup
    fn border_color(&self) -> Color {
        POPUP_BORDER.into()
    }

    fn update_with_state(&mut self, state: &AppState);

    /// Key Event Handler for the inner component of the popup,
    /// this method is called when the key event is not the escape key.
    fn inner_handle_key_event(&mut self, key: KeyEvent);

    /// Use this method to handle key events for the popup.
    ///
    /// It defers most of the key handling to the component implementing the popup, but
    /// it handles making the escape key close the popup.
    fn handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                action_tx.send(Action::Popup(PopupAction::Close)).ok();
            }
            _ => self.inner_handle_key_event(key),
        }
    }

    fn render_popup_border(&self, frame: &mut ratatui::Frame, area: Rect) -> Rect {
        let title = self.title();
        let instructions = self.instructions();

        // Draw border with title and instructions
        let border = Block::bordered()
            .title_top(title)
            .title_bottom(instructions)
            .border_style(Style::default().fg(self.border_color()));
        let component_area = border.inner(area);
        frame.render_widget(border, area);
        component_area
    }

    /// Use this method to handle rendering the popup.
    ///
    /// It draws a border with the given title and instructions and
    /// renders the component implementing popup.
    fn render_popup(&self, frame: &mut ratatui::Frame) {
        let area = self.area(frame.area());

        // clear the popup area
        frame.render_widget(Clear, area);

        self.render(frame, area);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupType {
    #[allow(dead_code)]
    Notification(Text<'static>),
    Playlist(Vec<Thing>),
}

impl PopupType {
    #[must_use]
    pub fn into_popup(
        self,
        state: &AppState,
        action_tx: UnboundedSender<Action>,
    ) -> Box<dyn Popup> {
        match self {
            Self::Notification(line) => Box::new(notification::Notification(line)) as _,
            Self::Playlist(items) => {
                Box::new(playlist::PlaylistSelector::new(state, action_tx, items)) as _
            }
        }
    }
}
