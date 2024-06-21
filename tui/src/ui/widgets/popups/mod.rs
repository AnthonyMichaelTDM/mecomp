pub mod notification;
pub mod playlist;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::Line,
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

    // TODO: implement a way for popups to listen to application state changes

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

    /// Use this method to handle rendering the popup.
    ///
    /// It draws a border with the given title and instructions and
    /// renders the component implementing popup.
    fn render_popup(&self, frame: &mut ratatui::Frame) {
        let title = self.title();
        let instructions = self.instructions();
        let area = self.area(frame.size());

        // clear the popup area
        frame.render_widget(Clear, area);

        // Draw border with title and instructions
        let border = Block::bordered()
            .title_top(title)
            .title_bottom(instructions)
            .border_style(Style::default().fg(self.border_color()));
        let component_area = border.inner(area);
        frame.render_widget(border, area);

        self.render(frame, component_area);
    }
}

pub enum PopupType {
    #[allow(dead_code)]
    Notification(Line<'static>),
    Playlist(Vec<Thing>),
}

impl PopupType {
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
