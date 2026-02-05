//! Module for the popup used to edit Dynamic Playlists.

use std::{cell::RefCell, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::{DynamicPlaylist, DynamicPlaylistChangeSet, RecordId};
use mecomp_storage::db::schemas::dynamic::query::Query;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Offset, Position, Rect},
    style::Style,
    text::Line,
    widgets::Block,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction},
    ui::{
        AppState,
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
        },
        components::ComponentRender,
        widgets::input_box::{InputBox, InputBoxState},
    },
};

use super::Popup;

/// The popup used to edit Dynamic Playlists.
pub struct DynamicPlaylistEditor {
    action_tx: UnboundedSender<Action>,
    dynamic_playlist_id: RecordId,
    name_input: RefCell<InputBoxState>,
    query_input: RefCell<InputBoxState>,
    focus: Focus,
}

impl DynamicPlaylistEditor {
    /// Create a new `DynamicPlaylistEditor`.
    #[must_use]
    pub fn new(action_tx: UnboundedSender<Action>, dynamic_playlist: DynamicPlaylist) -> Self {
        let mut name_input = InputBoxState::new();
        name_input.set_text(&dynamic_playlist.name);
        let mut query_input = InputBoxState::new();
        query_input.set_text(&dynamic_playlist.query.clone());

        Self {
            action_tx,
            dynamic_playlist_id: dynamic_playlist.id,
            name_input: RefCell::new(name_input),
            query_input: RefCell::new(query_input),
            focus: Focus::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Focus {
    #[default]
    Name,
    Query,
}

impl Focus {
    const fn next(self) -> Self {
        match self {
            Self::Name => Self::Query,
            Self::Query => Self::Name,
        }
    }
}

impl Popup for DynamicPlaylistEditor {
    fn title(&self) -> Line<'static> {
        Line::from("Edit Dynamic Playlist")
    }

    fn instructions(&self) -> Line<'static> {
        Line::from(" \u{23CE} : Save | Esc : Cancel ")
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        let height = 8;
        let width = u16::try_from(
            self.name_input
                .borrow()
                .text()
                .len()
                .max(self.query_input.borrow().text().len())
                .max(self.instructions().width())
                .max(self.title().width())
                + 5,
        )
        .unwrap_or(terminal_area.width)
        .min(terminal_area.width);

        let [_, vertical_area, _] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(height),
                Constraint::Fill(4),
            ])
            .split(terminal_area)
        else {
            panic!("Failed to split terminal area.");
        };

        let [_, horizontal_area, _] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Min(width),
                Constraint::Fill(1),
            ])
            .split(vertical_area)
        else {
            panic!("Failed to split terminal area.");
        };

        horizontal_area
    }

    fn update_with_state(&mut self, _: &AppState) {}

    fn inner_handle_key_event(&mut self, key: KeyEvent) {
        let query = Query::from_str(self.query_input.borrow().text()).ok();

        match (key.code, query) {
            (KeyCode::Tab, _) => {
                self.focus = self.focus.next();
            }
            (KeyCode::Enter, Some(query)) => {
                let change_set = DynamicPlaylistChangeSet {
                    new_name: Some(self.name_input.borrow().text().into()),
                    new_query: Some(query.to_string()),
                };

                self.action_tx
                    .send(Action::Library(LibraryAction::UpdateDynamicPlaylist(
                        self.dynamic_playlist_id.ulid(),
                        change_set,
                    )))
                    .ok();
                self.action_tx.send(Action::Popup(PopupAction::Close)).ok();
            }
            _ => match self.focus {
                Focus::Name => self.name_input.get_mut().handle_key_event(key),
                Focus::Query => self.query_input.get_mut().handle_key_event(key),
            },
        }
    }

    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            column, row, kind, ..
        } = mouse;
        let mouse_position = Position::new(column, row);
        let [name_area, query_area] = split_area(area, 3, 3);

        if name_area.contains(mouse_position) {
            if kind == MouseEventKind::Down(MouseButton::Left) {
                self.focus = Focus::Name;
            }
            self.name_input
                .get_mut()
                .handle_mouse_event(mouse, name_area);
        } else if query_area.contains(mouse_position) {
            if kind == MouseEventKind::Down(MouseButton::Left) {
                self.focus = Focus::Query;
            }
            self.query_input
                .get_mut()
                .handle_mouse_event(mouse, query_area);
        }
    }
}

fn split_area(area: Rect, name_height: u16, query_height: u16) -> [Rect; 2] {
    let [name_area, query_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(name_height),
            Constraint::Length(query_height),
        ])
        .areas(area);

    [name_area, query_area]
}

impl ComponentRender<Rect> for DynamicPlaylistEditor {
    fn render_border(&self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&self, frame: &mut Frame<'_>, area: Rect) {
        let [name_area, query_area] = split_area(area, 3, 3);

        let (name_color, query_color) = match self.focus {
            Focus::Name => ((*TEXT_HIGHLIGHT_ALT).into(), (*TEXT_NORMAL).into()),
            Focus::Query => ((*TEXT_NORMAL).into(), (*TEXT_HIGHLIGHT_ALT).into()),
        };
        let (name_border, query_border) = match self.focus {
            Focus::Name => ((*BORDER_FOCUSED).into(), (*BORDER_UNFOCUSED).into()),
            Focus::Query => ((*BORDER_UNFOCUSED).into(), (*BORDER_FOCUSED).into()),
        };

        let name_input = InputBox::new()
            .border(
                Block::bordered()
                    .title("Enter Name:")
                    .border_style(Style::default().fg(name_border)),
            )
            .text_color(name_color);
        frame.render_stateful_widget(name_input, name_area, &mut self.name_input.borrow_mut());

        let query_input = if Query::from_str(self.query_input.borrow().text()).is_ok() {
            InputBox::new()
                .border(
                    Block::bordered()
                        .title("Enter Query:")
                        .border_style(Style::default().fg(query_border)),
                )
                .text_color(query_color)
        } else {
            InputBox::new()
                .border(
                    Block::bordered()
                        .title("Invalid Query:")
                        .border_style(Style::default().fg(query_border)),
                )
                .text_color((*TEXT_HIGHLIGHT).into())
        };
        frame.render_stateful_widget(query_input, query_area, &mut self.query_input.borrow_mut());

        // update cursor position
        let position = match self.focus {
            Focus::Name => name_area + self.name_input.borrow().cursor_offset() + Offset::new(1, 1),
            Focus::Query => {
                query_area + self.query_input.borrow().cursor_offset() + Offset::new(1, 1)
            }
        };
        frame.set_cursor_position(position);
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{assert_buffer_eq, item_id, setup_test_terminal};

    use super::*;

    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use rstest::{fixture, rstest};

    #[fixture]
    fn state() -> AppState {
        AppState::default()
    }

    #[fixture]
    fn playlist() -> DynamicPlaylist {
        DynamicPlaylist {
            id: RecordId::new("dynamic", item_id()),
            name: "Test".into(),
            query: Query::from_str("title = \"foo \"").unwrap().to_string(),
        }
    }

    #[test]
    fn test_focus_next() {
        assert_eq!(Focus::Name.next(), Focus::Query);
        assert_eq!(Focus::Query.next(), Focus::Name);
    }

    #[rstest]
    // will give the popup at most 1/3 of the horizontal area,
    #[case::large((100,100), Rect::new(33, 18, 34,8))]
    // or at least 30 if it can
    #[case::small((40,8), Rect::new(5, 0, 30, 8))]
    #[case::small((30,8), Rect::new(0, 0, 30, 8))]
    // or whatever is left if the terminal is too small
    #[case::too_small((20,8), Rect::new(0, 0, 20, 8))]
    fn test_area(
        #[case] terminal_size: (u16, u16),
        #[case] expected_area: Rect,
        playlist: DynamicPlaylist,
    ) {
        let (_, area) = setup_test_terminal(terminal_size.0, terminal_size.1);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let editor = DynamicPlaylistEditor::new(tx, playlist);
        let area = editor.area(area);
        assert_eq!(area, expected_area);
    }

    #[rstest]
    fn test_key_event_handling(playlist: DynamicPlaylist) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut editor = DynamicPlaylistEditor::new(tx, playlist.clone());

        // Test tab changes focus
        assert_eq!(editor.focus, Focus::Name);
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Tab));
        assert_eq!(editor.focus, Focus::Query);

        // Test enter sends action
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv(),
            Some(Action::Library(LibraryAction::UpdateDynamicPlaylist(
                playlist.id.into(),
                DynamicPlaylistChangeSet {
                    new_name: Some(playlist.name.clone()),
                    new_query: Some(playlist.query.to_string())
                }
            )))
        );
        assert_eq!(rx.blocking_recv(), Some(Action::Popup(PopupAction::Close)));

        // other keys go to the focused input box
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('b')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('c')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('d')));
        assert_eq!(editor.query_input.borrow().text(), "title = \"foo \"abcd");
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Tab));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('e')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('f')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(editor.name_input.borrow().text(), "Testefg");
        // the backspace and delete keys work as intended
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Left));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Left));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Delete));
        assert_eq!(editor.name_input.borrow().text(), "Testeg");
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(editor.name_input.borrow().text(), "Testg");

        // Test invalid query does not send action
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Enter));
        let action = rx.try_recv();
        assert_eq!(action, Err(tokio::sync::mpsc::error::TryRecvError::Empty));
    }

    #[rstest]
    fn test_mouse_event_handling(playlist: DynamicPlaylist) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();

        let mut editor = DynamicPlaylistEditor::new(tx, playlist);
        let area = Rect::new(0, 0, 50, 10);

        // Test clicking name area changes focus
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::empty(),
        };
        editor.inner_handle_mouse_event(mouse_event, area);
        assert_eq!(editor.focus, Focus::Name);
    }

    #[rstest]
    fn test_render(playlist: DynamicPlaylist) {
        let (mut terminal, _) = setup_test_terminal(30, 8);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let editor = DynamicPlaylistEditor::new(tx, playlist);
        let buffer = terminal
            .draw(|frame| editor.render_popup(frame))
            .unwrap()
            .buffer
            .clone();

        let expected = Buffer::with_lines([
            "┌Edit Dynamic Playlist───────┐",
            "│┌Enter Name:───────────────┐│",
            "││Test                      ││",
            "│└──────────────────────────┘│",
            "│┌Enter Query:──────────────┐│",
            "││title = \"foo \"            ││",
            "│└──────────────────────────┘│",
            "└ ⏎ : Save | Esc : Cancel ───┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }
}
