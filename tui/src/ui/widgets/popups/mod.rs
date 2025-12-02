pub mod dynamic;
pub mod notification;
pub mod playlist;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::{DynamicPlaylist, PlaylistBrief, RecordId};
use ratatui::{
    layout::Position,
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Clear},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, PopupAction},
    ui::{AppState, colors::POPUP_BORDER, components::ComponentRender},
};

pub trait Popup: for<'a> ComponentRender<Rect> + Send + Sync {
    fn title(&self) -> Line<'_>;
    fn instructions(&self) -> Line<'_>;
    /// The area needed for the popup to render.
    fn area(&self, terminal_area: Rect) -> Rect;

    /// override this method to change the border color of the popup
    fn border_color(&self) -> Color {
        (*POPUP_BORDER).into()
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

    /// Mouse Event Handler for the inner component of the popup,
    /// this method is called when the mouse event is inside the popup area.
    ///
    /// The default behavior is to close the popup when the mouse is clicked.
    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect);

    /// popup, but
    /// it handles making the escape key close the popup.
    fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: UnboundedSender<Action>,
    ) {
        if area.contains(Position::new(mouse.column, mouse.row)) {
            return self.inner_handle_mouse_event(mouse, area);
        }

        // Close the popup when the mouse is clicked outside the popup
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            action_tx.send(Action::Popup(PopupAction::Close)).ok();
        }
    }

    fn render_popup_border(&self, frame: &mut ratatui::Frame<'_>, area: Rect) -> Rect {
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
    fn render_popup(&self, frame: &mut ratatui::Frame<'_>) {
        let area = self.area(frame.area());

        // clear the popup area
        frame.render_widget(Clear, area);

        self.render(frame, area);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupType {
    Notification(Text<'static>),
    Playlist(Vec<RecordId>),
    PlaylistEditor(PlaylistBrief),
    DynamicPlaylistEditor(DynamicPlaylist),
}

impl PopupType {
    #[must_use]
    pub fn into_popup(
        self,
        state: &AppState,
        action_tx: UnboundedSender<Action>,
    ) -> Box<dyn Popup> {
        match self {
            Self::Notification(line) => {
                Box::new(notification::Notification::new(line, action_tx)) as _
            }
            Self::Playlist(items) => {
                Box::new(playlist::PlaylistSelector::new(state, action_tx, items)) as _
            }
            Self::PlaylistEditor(playlist) => Box::new(playlist::PlaylistEditor::new(
                state,
                action_tx,
                playlist.id.ulid(),
                &playlist.name,
            )) as _,
            Self::DynamicPlaylistEditor(playlist) => Box::new(dynamic::DynamicPlaylistEditor::new(
                state, action_tx, playlist,
            )) as _,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::state_with_everything;

    use super::*;

    #[test]
    fn test_popup_type_into_popup() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();

        // Test notification popup
        let notification = PopupType::Notification(Text::raw("Test notification"));
        let popup = notification.into_popup(&state, tx.clone());
        assert_eq!(popup.title().to_string(), "Notification");

        // Test playlist selector popup
        let items = vec![];
        let playlist = PopupType::Playlist(items);
        let popup = playlist.into_popup(&state, tx.clone());
        assert_eq!(popup.title().to_string(), "Select a Playlist");

        // Test playlist editor popup
        let playlist = PopupType::PlaylistEditor(state.library.playlists[0].clone());
        let popup = playlist.into_popup(&state, tx.clone());
        assert_eq!(popup.title().to_string(), "Rename Playlist");

        // Test dynamic playlist editor popup
        let dynamic = PopupType::DynamicPlaylistEditor(state.library.dynamic_playlists[0].clone());
        let popup = dynamic.into_popup(&state, tx);
        assert_eq!(popup.title().to_string(), "Edit Dynamic Playlist");
    }
}
