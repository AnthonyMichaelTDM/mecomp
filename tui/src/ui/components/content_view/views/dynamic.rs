use std::{str::FromStr, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::dynamic::{query::Query, DynamicPlaylist};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction, ViewAction},
    ui::{
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
        },
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::{
            input_box::{self, InputBox},
            popups::PopupType,
            tree::{state::CheckTreeState, CheckTree},
        },
        AppState,
    },
};

use super::{
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        construct_start_radio_action, create_dynamic_playlist_tree_leaf, create_song_tree_leaf,
    },
    sort_mode::{NameSort, SongSort},
    traits::SortMode,
    DynamicPlaylistViewProps,
};

/// A Query Building interface for Dynamic Playlists
///
/// Currently just a wrapper around an `InputBox`,
/// but I want it to be more like a advanced search builder from something like airtable or a research database.
pub struct QueryBuilder {
    inner: InputBox,
}

impl QueryBuilder {
    #[must_use]
    pub fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            inner: InputBox::new(state, action_tx),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.inner.text()
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        self.inner.handle_key_event(key);
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        self.inner.handle_mouse_event(mouse, area);
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct DynamicView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<DynamicPlaylistViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// sort mode
    sort_mode: SongSort,
}

impl Component for DynamicView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.dynamic_playlist.clone(),
            tree_state: Mutex::new(CheckTreeState::default()),
            sort_mode: SongSort::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.dynamic_playlist {
            let mut props = props.clone();
            self.sort_mode.sort_items(&mut props.songs);

            Self {
                props: Some(props),
                tree_state: Mutex::new(CheckTreeState::default()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &'static str {
        "Dynamic Playlist View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(
                        self.props
                            .as_ref()
                            .map_or(0, |p| p.songs.len().saturating_sub(1)),
                        |c| c.saturating_sub(10),
                    )
                });
            }
            KeyCode::Up => {
                self.tree_state.lock().unwrap().key_up();
            }
            KeyCode::PageDown => {
                self.tree_state
                    .lock()
                    .unwrap()
                    .select_relative(|current| current.map_or(0, |c| c.saturating_add(10)));
            }
            KeyCode::Down => {
                self.tree_state.lock().unwrap().key_down();
            }
            KeyCode::Left => {
                self.tree_state.lock().unwrap().key_left();
            }
            KeyCode::Right => {
                self.tree_state.lock().unwrap().key_right();
            }
            KeyCode::Char(' ') => {
                self.tree_state.lock().unwrap().key_space();
            }
            // Change sort mode
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.next();
                if let Some(props) = &mut self.props {
                    self.sort_mode.sort_items(&mut props.songs);
                }
            }
            KeyCode::Char('S') => {
                self.sort_mode = self.sort_mode.prev();
                if let Some(props) = &mut self.props {
                    self.sort_mode.sort_items(&mut props.songs);
                }
            }
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let selected_things = self.tree_state.lock().unwrap().get_selected_thing();
                    if let Some(thing) = selected_things {
                        self.action_tx
                            .send(Action::ActiveView(ViewAction::Set(thing.into())))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, add them to the queue, otherwise send the whole playlist to the queue
            KeyCode::Char('q') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_add_to_queue_action(
                    checked_things,
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, start radio from checked items, otherwise start radio from the playlist
            KeyCode::Char('r') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) =
                    construct_start_radio_action(checked_things, self.props.as_ref().map(|p| &p.id))
                {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to the playlist, otherwise add the whole playlist to the playlist
            KeyCode::Char('p') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_add_to_playlist_action(
                    checked_things,
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // "e" key to edit the name/query of the dynamic playlist
            KeyCode::Char('e') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(
                            PopupType::DynamicPlaylistEditor(props.dynamic_playlist.clone()),
                        )))
                        .unwrap();
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));
        let [_, content_area] = split_area(area);
        let content_area = content_area.inner(Margin::new(0, 1));

        let result = self
            .tree_state
            .lock()
            .unwrap()
            .handle_mouse_event(mouse, content_area);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [info_area, content_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(4)])
        .split(area)
    else {
        panic!("Failed to split dynamic playlist view area")
    };

    [info_area, content_area]
}

impl ComponentRender<RenderProps> for DynamicView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Dynamic Playlist View".to_string(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            let content_area = border.inner(props.area);

            // split content area to make room for dynamic playlist info
            let [info_area, content_area] = split_area(content_area);

            // render the dynamic playlist info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        state.dynamic_playlist.name.to_string(),
                        Style::default().bold(),
                    )),
                    Line::from(vec![
                        Span::raw("Songs: "),
                        Span::styled(state.songs.len().to_string(), Style::default().italic()),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format_duration(&state.songs.iter().map(|s| s.runtime).sum()),
                            Style::default().italic(),
                        ),
                    ]),
                    Line::from(Span::styled(
                        state.dynamic_playlist.query.to_string(),
                        Style::default().italic(),
                    )),
                ])
                .alignment(Alignment::Center),
                info_area,
            );

            // draw an additional border around the content area to display additional instructions
            let border = Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title_top("q: add to queue | r: start radio | p: add to playlist")
                .title_bottom("s/S: sort | e: edit")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            let content_area = border.inner(content_area);

            // draw an additional border around the content area to indicate whether operations will be performed on the entire item, or just the checked items
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top(Line::from(vec![
                    Span::raw("Performing operations on "),
                    Span::raw(
                        if self
                            .tree_state
                            .lock()
                            .unwrap()
                            .get_checked_things()
                            .is_empty()
                        {
                            "entire dynamic playlist"
                        } else {
                            "checked items"
                        },
                    )
                    .fg(TEXT_HIGHLIGHT),
                ]))
                .italic()
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            border.inner(content_area)
        } else {
            let border = Block::bordered()
                .title_top("Dynamic Playlist View")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create list to hold playlist songs
            let items = state
                .songs
                .iter()
                .map(create_song_tree_leaf)
                .collect::<Vec<_>>();

            // render the playlist songs
            frame.render_stateful_widget(
                CheckTree::new(&items)
                    .unwrap()
                    .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
                    .experimental_scrollbar(Some(Scrollbar::new(
                        ScrollbarOrientation::VerticalRight,
                    ))),
                props.area,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            let text = "No active dynamic playlist";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}

pub struct LibraryDynamicView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// Dynamic Playlist Name Input Box
    name_input_box: InputBox,
    /// Dynamic Playlist Query Input Box
    query_builder: QueryBuilder,
    /// What is currently focused
    /// Note: name and query input boxes are only visible when one of them is focused
    focus: Focus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    NameInput,
    QueryInput,
    Tree,
}

#[derive(Debug)]
pub struct Props {
    pub dynamics: Box<[DynamicPlaylist]>,
    sort_mode: NameSort<DynamicPlaylist>,
}

impl From<&AppState> for Props {
    fn from(state: &AppState) -> Self {
        let mut dynamics = state.library.dynamic_playlists.clone();
        let sort_mode = NameSort::default();
        sort_mode.sort_items(&mut dynamics);
        Self {
            dynamics,
            sort_mode,
        }
    }
}

impl Component for LibraryDynamicView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            name_input_box: InputBox::new(state, action_tx.clone()),
            query_builder: QueryBuilder::new(state, action_tx.clone()),
            focus: Focus::Tree,
            action_tx,
            props: Props::from(state),
            tree_state: Mutex::new(CheckTreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let tree_state = if state.active_view == ActiveView::DynamicPlaylists {
            self.tree_state
        } else {
            Mutex::new(CheckTreeState::default())
        };

        Self {
            props: Props::from(state),
            tree_state,
            ..self
        }
    }

    fn name(&self) -> &'static str {
        "Library Dynamic Playlists View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        // this page has 2 distinct "modes",
        // one for navigating the tree when the input box is not visible
        // one for interacting with the input box when it is visible
        if self.focus == Focus::NameInput {
            match key.code {
                // if the user presses Enter, we prompt the user for the query
                KeyCode::Enter => {
                    if self.name_input_box.text().is_empty() {
                        self.focus = Focus::Tree;
                    } else {
                        self.focus = Focus::QueryInput;
                    }
                }
                // defer to the input box
                _ => {
                    self.name_input_box.handle_key_event(key);
                }
            }
        } else if self.focus == Focus::QueryInput {
            match key.code {
                // if the user presses Enter, we try to create a new playlist with the given name and query
                KeyCode::Enter => {
                    let name = self.name_input_box.text();
                    let query = self.query_builder.text();

                    if !name.is_empty() && !query.is_empty() {
                        let query = Query::from_str(query);

                        if let Ok(query) = query {
                            self.action_tx
                                .send(Action::Library(LibraryAction::CreateDynamicPlaylist(
                                    name.to_string(),
                                    query,
                                )))
                                .unwrap();
                        } else {
                            // TODO: Find a better way to notify the user that the query is invalid
                            self.action_tx
                                .send(Action::Popup(PopupAction::Open(PopupType::Notification(
                                    "Invalid Query".into(),
                                ))))
                                .unwrap();
                            return;
                        }
                    }

                    self.focus = Focus::Tree;
                }
                // defer to the input box
                _ => {
                    self.query_builder.handle_key_event(key);
                }
            }
        } else {
            match key.code {
                // arrow keys
                KeyCode::PageUp => {
                    self.tree_state.lock().unwrap().select_relative(|current| {
                        current.map_or(self.props.dynamics.len() - 1, |c| c.saturating_sub(10))
                    });
                }
                KeyCode::Up => {
                    self.tree_state.lock().unwrap().key_up();
                }
                KeyCode::PageDown => {
                    self.tree_state
                        .lock()
                        .unwrap()
                        .select_relative(|current| current.map_or(0, |c| c.saturating_add(10)));
                }
                KeyCode::Down => {
                    self.tree_state.lock().unwrap().key_down();
                }
                KeyCode::Left => {
                    self.tree_state.lock().unwrap().key_left();
                }
                KeyCode::Right => {
                    self.tree_state.lock().unwrap().key_right();
                }
                // Enter key opens selected view
                KeyCode::Enter => {
                    if self.tree_state.lock().unwrap().toggle_selected() {
                        let things = self.tree_state.lock().unwrap().get_selected_thing();

                        if let Some(thing) = things {
                            self.action_tx
                                .send(Action::ActiveView(ViewAction::Set(thing.into())))
                                .unwrap();
                        }
                    }
                }
                // Change sort mode
                KeyCode::Char('s') => {
                    self.props.sort_mode = self.props.sort_mode.next();
                    self.props.sort_mode.sort_items(&mut self.props.dynamics);
                }
                KeyCode::Char('S') => {
                    self.props.sort_mode = self.props.sort_mode.prev();
                    self.props.sort_mode.sort_items(&mut self.props.dynamics);
                }
                // "n" key to create a new playlist
                KeyCode::Char('n') => {
                    self.focus = Focus::NameInput;
                }
                // "d" key to delete the selected playlist
                KeyCode::Char('d') => {
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::Library(LibraryAction::RemoveDynamicPlaylist(thing)))
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));

        if self.focus == Focus::Tree {
            let area = Rect {
                y: area.y + 1,
                height: area.height - 1,
                ..area
            };

            let result = self
                .tree_state
                .lock()
                .unwrap()
                .handle_mouse_event(mouse, area);
            if let Some(action) = result {
                self.action_tx.send(action).unwrap();
            }
        } else {
            let [input_box_area, query_builder_area, content_area] = lib_split_area(area);
            let content_area = Rect {
                y: content_area.y + 1,
                height: content_area.height - 1,
                ..content_area
            };

            if input_box_area.contains(mouse_position) {
                if kind == MouseEventKind::Down(MouseButton::Left) {
                    self.focus = Focus::NameInput;
                }
                self.name_input_box
                    .handle_mouse_event(mouse, input_box_area);
            } else if query_builder_area.contains(mouse_position) {
                if kind == MouseEventKind::Down(MouseButton::Left) {
                    self.focus = Focus::QueryInput;
                }
                self.query_builder
                    .handle_mouse_event(mouse, query_builder_area);
            } else if content_area.contains(mouse_position)
                && kind == MouseEventKind::Down(MouseButton::Left)
            {
                self.focus = Focus::Tree;
            }
        }
    }
}

fn lib_split_area(area: Rect) -> [Rect; 3] {
    let [input_box_area, query_builder_area, content_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area)
    else {
        panic!("Failed to split library dynamic playlists view area");
    };
    [input_box_area, query_builder_area, content_area]
}

impl ComponentRender<RenderProps> for LibraryDynamicView {
    fn render_border(&self, frame: &mut Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // render primary border
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled(
                    "Library Dynamic Playlists".to_string(),
                    Style::default().bold(),
                ),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(if self.focus == Focus::Tree {
                " \u{23CE} : Open | ←/↑/↓/→: Navigate | s/S: change sort"
            } else {
                ""
            })
            .border_style(border_style);
        let content_area = border.inner(props.area);
        frame.render_widget(border, props.area);

        // render input box (if visible)
        let content_area = if self.focus == Focus::Tree {
            content_area
        } else {
            // split content area to make room for the input box
            let [input_box_area, query_builder_area, content_area] = lib_split_area(content_area);

            let (name_text_color, query_text_color) = match self.focus {
                Focus::NameInput => (TEXT_HIGHLIGHT_ALT.into(), TEXT_HIGHLIGHT.into()),
                Focus::QueryInput => (TEXT_HIGHLIGHT.into(), TEXT_HIGHLIGHT_ALT.into()),
                Focus::Tree => (TEXT_NORMAL.into(), TEXT_NORMAL.into()),
            };

            let (name_border_color, query_border_color) = match self.focus {
                Focus::NameInput => (BORDER_FOCUSED.into(), BORDER_UNFOCUSED.into()),
                Focus::QueryInput => (BORDER_UNFOCUSED.into(), BORDER_FOCUSED.into()),
                Focus::Tree => (BORDER_UNFOCUSED.into(), BORDER_UNFOCUSED.into()),
            };

            let (name_show_cursor, query_show_cursor) = match self.focus {
                Focus::NameInput => (true, false),
                Focus::QueryInput => (false, true),
                Focus::Tree => (false, false),
            };

            // render the name input box
            self.name_input_box.render(
                frame,
                input_box::RenderProps {
                    area: input_box_area,
                    text_color: name_text_color,
                    border: Block::bordered()
                        .title("Enter Name:")
                        .border_style(Style::default().fg(name_border_color)),
                    show_cursor: name_show_cursor,
                },
            );

            // render the query input box
            self.query_builder.inner.render(
                frame,
                input_box::RenderProps {
                    area: query_builder_area,
                    text_color: query_text_color,
                    border: Block::bordered()
                        .title("Enter Query:")
                        .border_style(Style::default().fg(query_border_color)),
                    show_cursor: query_show_cursor,
                },
            );

            content_area
        };

        // draw additional border around content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP)
            .title_top(match self.focus {
                Focus::NameInput => " \u{23CE} : Set (cancel if empty)",
                Focus::QueryInput => " \u{23CE} : Create (cancel if empty)",
                Focus::Tree => "n: new dynamic | d: delete dynamic",
            })
            .border_style(border_style);
        let area = border.inner(content_area);
        frame.render_widget(border, content_area);

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut Frame, props: RenderProps) {
        // create a tree for the playlists
        let items = self
            .props
            .dynamics
            .iter()
            .map(create_dynamic_playlist_tree_leaf)
            .collect::<Vec<_>>();

        // render the playlists
        frame.render_stateful_widget(
            CheckTree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
                // we want this to be rendered like a normal tree, not a check tree, so we don't show the checkboxes
                .node_unchecked_symbol("▪ ")
                .node_checked_symbol("▪ ")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
