//! Module for the popup used to edit Dynamic Playlists.

use std::str::FromStr;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_storage::db::schemas::{
    dynamic::{query::Query, DynamicPlaylist, DynamicPlaylistChangeSet},
    Thing,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::Style,
    text::Line,
    widgets::Block,
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction},
    ui::{
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
        },
        components::{Component, ComponentRender},
        widgets::input_box::{InputBox, RenderProps},
        AppState,
    },
};

use super::Popup;

/// The popup used to edit Dynamic Playlists.
pub struct DynamicPlaylistEditor {
    action_tx: UnboundedSender<Action>,
    dynamic_playlist_id: Thing,
    name_input: InputBox,
    query_input: InputBox,
    focus: Focus,
}

impl DynamicPlaylistEditor {
    /// Create a new `DynamicPlaylistEditor`.
    #[must_use]
    pub fn new(
        state: &AppState,
        action_tx: UnboundedSender<Action>,
        dynamic_playlist: DynamicPlaylist,
    ) -> Self {
        let mut name_input = InputBox::new(state, action_tx.clone());
        name_input.set_text(&dynamic_playlist.name);
        let mut query_input = InputBox::new(state, action_tx.clone());
        query_input.set_text(&dynamic_playlist.query.to_string());

        Self {
            action_tx,
            dynamic_playlist_id: dynamic_playlist.id.into(),
            name_input,
            query_input,
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
    fn title(&self) -> Line {
        Line::from("Edit Dynamic Playlist")
    }

    fn instructions(&self) -> Line {
        Line::from("Press Enter to save, Esc to cancel.")
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        let height = 8;
        let width = u16::try_from(
            self.name_input
                .text()
                .len()
                .max(self.query_input.text().len())
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
        let query = Query::from_str(self.query_input.text()).ok();

        match (key.code, query) {
            (KeyCode::Tab, _) => {
                self.focus = self.focus.next();
            }
            (KeyCode::Enter, Some(query)) => {
                let change_set = DynamicPlaylistChangeSet {
                    name: Some(self.name_input.text().into()),
                    query: Some(query),
                };

                self.action_tx
                    .send(Action::Library(LibraryAction::UpdateDynamicPlaylist(
                        self.dynamic_playlist_id.clone(),
                        change_set,
                    )))
                    .ok();
                self.action_tx.send(Action::Popup(PopupAction::Close)).ok();
            }
            _ => match self.focus {
                Focus::Name => self.name_input.handle_key_event(key),
                Focus::Query => self.query_input.handle_key_event(key),
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
            self.name_input.handle_mouse_event(mouse, name_area);
        } else if query_area.contains(mouse_position) {
            if kind == MouseEventKind::Down(MouseButton::Left) {
                self.focus = Focus::Query;
            }
            self.query_input.handle_mouse_event(mouse, query_area);
        }
    }
}

fn split_area(area: Rect, name_height: u16, query_height: u16) -> [Rect; 2] {
    let [name_area, query_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(name_height),
            Constraint::Length(query_height),
        ])
        .split(area)
    else {
        panic!("Failed to split area.");
    };

    [name_area, query_area]
}

impl ComponentRender<Rect> for DynamicPlaylistEditor {
    fn render_border(&self, frame: &mut Frame, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&self, frame: &mut Frame, area: Rect) {
        let [name_area, query_area] = split_area(area, 3, 3);

        let (name_color, query_color) = match self.focus {
            Focus::Name => (TEXT_HIGHLIGHT_ALT.into(), TEXT_NORMAL.into()),
            Focus::Query => (TEXT_NORMAL.into(), TEXT_HIGHLIGHT_ALT.into()),
        };
        let (name_border, query_border) = match self.focus {
            Focus::Name => (BORDER_FOCUSED.into(), BORDER_UNFOCUSED.into()),
            Focus::Query => (BORDER_UNFOCUSED.into(), BORDER_FOCUSED.into()),
        };

        self.name_input.render(
            frame,
            RenderProps {
                border: Block::bordered()
                    .title("Enter Name:")
                    .border_style(Style::default().fg(name_border)),
                area: name_area,
                text_color: name_color,
                show_cursor: self.focus == Focus::Name,
            },
        );

        if Query::from_str(self.query_input.text()).is_ok() {
            self.query_input.render(
                frame,
                RenderProps {
                    border: Block::bordered()
                        .title("Enter Query:")
                        .border_style(Style::default().fg(query_border)),
                    area: query_area,
                    text_color: query_color,
                    show_cursor: self.focus == Focus::Query,
                },
            );
        } else {
            self.query_input.render(
                frame,
                RenderProps {
                    border: Block::bordered()
                        .title("Invalid Query:")
                        .border_style(Style::default().fg(query_border)),
                    area: query_area,
                    text_color: TEXT_HIGHLIGHT.into(),
                    show_cursor: self.focus == Focus::Query,
                },
            );
        }
    }
}
