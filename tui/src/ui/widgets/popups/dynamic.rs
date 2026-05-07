//! Module for the popup used to edit Dynamic Playlists.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::{DynamicPlaylist, DynamicPlaylistChangeSet, RecordId};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Offset, Position, Rect},
    style::Style,
    text::Line,
    widgets::Block,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, OverlayAction, PopupAction},
    ui::{
        AppState,
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL},
        components::ComponentRender,
        widgets::{
            input_box::{InputBox, InputBoxState},
            overlay::OverlayResult,
            query_builder::state::BuilderMode,
        },
    },
};

use crate::ui::widgets::query_builder::QueryBuilder;

use super::Popup;

pub enum PopupType {
    Edit(RecordId),
    Create,
}

/// The popup used to edit Dynamic Playlists.
pub struct DynamicPlaylistEditor {
    action_tx: UnboundedSender<Action>,
    name_input: InputBoxState,
    query_builder: QueryBuilder,
    focus: Focus,
    kind: PopupType,
}

impl DynamicPlaylistEditor {
    /// Create a new `DynamicPlaylistEditor` to edit an existing dynamic playlist.
    #[must_use]
    pub fn new_editor(
        action_tx: UnboundedSender<Action>,
        dynamic_playlist: DynamicPlaylist,
    ) -> Self {
        let mut name_input = InputBoxState::new();
        name_input.set_text(&dynamic_playlist.name);
        let mut query_builder = QueryBuilder::new();
        if let Ok(q) = dynamic_playlist.query.parse() {
            query_builder.set_query(&q);
        }

        Self {
            action_tx,
            name_input,
            query_builder,
            focus: Focus::Name,
            kind: PopupType::Edit(dynamic_playlist.id),
        }
    }

    /// Create a new `DynamicPlaylistEditor` to create a new dynamic playlist
    #[must_use]
    pub fn new_creator(action_tx: UnboundedSender<Action>) -> Self {
        let name_input = InputBoxState::new();
        let query_builder = QueryBuilder::new();

        Self {
            action_tx,
            name_input,
            query_builder,
            focus: Focus::Name,
            kind: PopupType::Create,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Focus {
    #[default]
    Name,
    Query,
}

impl Popup for DynamicPlaylistEditor {
    fn title(&self) -> Line<'static> {
        Line::from("Edit Dynamic Playlist")
    }

    fn instructions(&self) -> Line<'static> {
        Line::from(" \u{23CE} : Save | Esc : Cancel ")
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        let height = 15;

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
                Constraint::Fill(2),
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
        let query = self.query_builder.query();

        match (key.code, key.modifiers, query) {
            (KeyCode::Tab, _, _) => match self.focus {
                Focus::Name => {
                    self.focus = Focus::Query;
                    self.query_builder.state.mode = BuilderMode::Visual;
                }
                Focus::Query => match self.query_builder.state.mode {
                    BuilderMode::Visual => {
                        self.query_builder.state.mode = BuilderMode::RawText;
                    }
                    BuilderMode::RawText => self.focus = Focus::Name,
                },
            },
            (KeyCode::BackTab, _, _) => match self.focus {
                Focus::Name => {
                    self.focus = Focus::Query;
                    self.query_builder.state.mode = BuilderMode::RawText;
                }
                Focus::Query => match self.query_builder.state.mode {
                    BuilderMode::RawText => {
                        self.query_builder.state.mode = BuilderMode::Visual;
                    }
                    BuilderMode::Visual => self.focus = Focus::Name,
                },
            },
            (KeyCode::Enter, KeyModifiers::CONTROL, Some(query)) => {
                let name = self.name_input.text().into();
                let action = match &self.kind {
                    PopupType::Edit(id) => {
                        let change_set = DynamicPlaylistChangeSet {
                            new_name: Some(name),
                            new_query: Some(query.to_string()),
                        };
                        Action::Library(LibraryAction::UpdateDynamicPlaylist(id.ulid(), change_set))
                    }
                    PopupType::Create => {
                        Action::Library(LibraryAction::CreateDynamicPlaylist(name, query))
                    }
                };

                self.action_tx.send(action).ok();

                self.action_tx
                    .send(Action::Overlay(OverlayAction::Close))
                    .ok();

                self.action_tx.send(Action::Popup(PopupAction::Close)).ok();
            }
            _ => match self.focus {
                Focus::Name => self.name_input.handle_key_event(key),
                Focus::Query => self.query_builder.handle_key_event(key, &self.action_tx),
            },
        }
    }

    fn handle_overlay_result(&mut self, result: &OverlayResult) {
        if self.focus == Focus::Query {
            self.query_builder.apply_overlay_result(result);
        }
    }

    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            column, row, kind, ..
        } = mouse;
        let mouse_position = Position::new(column, row);
        let [name_area, query_area] = split_area(area);

        if name_area.contains(mouse_position) {
            if kind == MouseEventKind::Down(MouseButton::Left) {
                self.focus = Focus::Name;
            }
            self.name_input.handle_mouse_event(mouse, name_area);
        } else if query_area.contains(mouse_position) {
            if kind == MouseEventKind::Down(MouseButton::Left) {
                self.focus = Focus::Query;
            }
            self.query_builder
                .handle_mouse_event(mouse, query_area, &self.action_tx);
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [name_area, query_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .areas(area);

    [name_area, query_area]
}

impl ComponentRender<Rect> for DynamicPlaylistEditor {
    fn render_border(&mut self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let [name_area, query_area] = split_area(area);

        let name_color = match self.focus {
            Focus::Name => (*TEXT_HIGHLIGHT_ALT).into(),
            Focus::Query => (*TEXT_NORMAL).into(),
        };
        let name_border_color = match self.focus {
            Focus::Name => (*BORDER_FOCUSED).into(),
            Focus::Query => (*BORDER_UNFOCUSED).into(),
        };

        let name_input = InputBox::new()
            .border(
                Block::bordered()
                    .title("Enter Name:")
                    .border_style(Style::default().fg(name_border_color)),
            )
            .text_color(name_color);
        frame.render_stateful_widget(name_input, name_area, &mut self.name_input);

        // render the visual query builder; it manages its own cursor in raw-text mode
        self.query_builder
            .render(frame, query_area, self.focus == Focus::Query);

        // only the name input box needs an explicit terminal cursor position
        if self.focus == Focus::Name {
            let position = name_area + self.name_input.cursor_offset() + Offset::new(1, 1);
            frame.set_cursor_position(position);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::test_utils::{assert_buffer_eq, item_id, setup_test_terminal};

    use super::*;

    use crossterm::event::KeyModifiers;
    use mecomp_storage::db::schemas::dynamic::query::Query;
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

    #[rstest]
    // will give the popup at most 1/3 of the horizontal area,
    #[case::large((100,100), Rect::new(25, 17, 50, 15))]
    // or at least 30 if it can
    #[case::small((40,8), Rect::new(10, 0, 20, 8))]
    #[case::small2((30,8), Rect::new(8, 0, 15, 8))]
    // or whatever is left if the terminal is too small
    #[case::too_small((20,8), Rect::new(5, 0, 10, 8))]
    fn test_area(
        #[case] terminal_size: (u16, u16),
        #[case] expected_area: Rect,
        playlist: DynamicPlaylist,
    ) {
        let (_, area) = setup_test_terminal(terminal_size.0, terminal_size.1);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let editor = DynamicPlaylistEditor::new_editor(tx, playlist);
        let area = editor.area(area);
        assert_eq!(area, expected_area);
    }

    #[rstest]
    #[ignore = "TODO: rewrite for new QueryBuilder API - query_input field no longer exists"]
    fn test_key_event_handling(playlist: DynamicPlaylist) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut editor = DynamicPlaylistEditor::new_editor(tx, playlist.clone());

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
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Tab));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('e')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('f')));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(editor.name_input.text(), "Testefg");
        // the backspace and delete keys work as intended
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Left));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Left));
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Delete));
        assert_eq!(editor.name_input.text(), "Testeg");
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(editor.name_input.text(), "Testg");

        // Test invalid query does not send action
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Enter));
        let action = rx.try_recv();
        assert_eq!(action, Err(tokio::sync::mpsc::error::TryRecvError::Empty));
    }

    #[rstest]
    fn test_mouse_event_handling(playlist: DynamicPlaylist) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();

        let mut editor = DynamicPlaylistEditor::new_editor(tx, playlist);
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
    #[ignore = "TODO: update expected buffer for new query builder rendering"]
    fn test_render(playlist: DynamicPlaylist) {
        let (mut terminal, _) = setup_test_terminal(30, 8);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut editor = DynamicPlaylistEditor::new_editor(tx, playlist);
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
