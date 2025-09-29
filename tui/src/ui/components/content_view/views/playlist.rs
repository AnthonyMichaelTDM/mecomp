//! Views for both a single playlist, and the library of playlists.

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::playlist::PlaylistBrief;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction, ViewAction},
    ui::{
        AppState,
        colors::{BORDER_FOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL, border_color},
        components::{Component, ComponentRender, RenderProps, content_view::ActiveView},
        widgets::{
            input_box::{self, InputBox},
            popups::PopupType,
            tree::{CheckTree, state::CheckTreeState},
        },
    },
};

use super::{
    PlaylistViewProps,
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        construct_start_radio_action, create_playlist_tree_leaf, create_song_tree_leaf,
    },
    sort_mode::{NameSort, SongSort},
    traits::SortMode,
};

#[allow(clippy::module_name_repetitions)]
pub struct PlaylistView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<PlaylistViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// sort mode
    sort_mode: SongSort,
}

impl Component for PlaylistView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.playlist.clone(),
            tree_state: Mutex::new(CheckTreeState::default()),
            sort_mode: SongSort::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.playlist {
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
        "Playlist View"
    }

    #[allow(clippy::too_many_lines)]
    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    let first = self
                        .props
                        .as_ref()
                        .map_or(0, |p| p.songs.len().saturating_sub(1));
                    current.map_or(first, |c| c.saturating_sub(10))
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
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
                }
            }
            KeyCode::Char('S') => {
                self.sort_mode = self.sort_mode.prev();
                if let Some(props) = &mut self.props {
                    self.sort_mode.sort_items(&mut props.songs);
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
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
            // if there are checked items, remove them from the playlist, otherwise remove the whole playlist
            KeyCode::Char('d') => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = self.props.as_ref().and_then(|props| {
                    let id = props.id.clone();
                    if things.is_empty() {
                        self.tree_state
                            .lock()
                            .unwrap()
                            .get_selected_thing()
                            .map(|thing| LibraryAction::RemoveSongsFromPlaylist(id, vec![thing]))
                    } else {
                        Some(LibraryAction::RemoveSongsFromPlaylist(id, things))
                    }
                }) {
                    self.action_tx.send(Action::Library(action)).unwrap();
                }
            }
            // edit the playlist name
            KeyCode::Char('e') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::PlaylistEditor(
                            props.playlist.clone().into(),
                        ))))
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
            .handle_mouse_event(mouse, content_area, false);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [info_area, content_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area)
    else {
        panic!("Failed to split playlist view area")
    };

    [info_area, content_area]
}

impl ComponentRender<RenderProps> for PlaylistView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Playlist View".to_string(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            let content_area = border.inner(props.area);

            // split content area to make room for playlist info
            let [info_area, content_area] = split_area(content_area);

            // render the playlist info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        state.playlist.name.to_string(),
                        Style::default().bold(),
                    )),
                    Line::from(vec![
                        Span::raw("Songs: "),
                        Span::styled(
                            state.playlist.song_count.to_string(),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format_duration(&state.playlist.runtime),
                            Style::default().italic(),
                        ),
                    ]),
                ])
                .alignment(Alignment::Center),
                info_area,
            );

            // draw an additional border around the content area to display additional instructions
            let border = Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title_top("q: add to queue | r: start radio | p: add to playlist")
                .title_bottom("s/S: sort | d: remove selected | e: edit")
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
                            "entire playlist"
                        } else {
                            "checked items"
                        },
                    )
                    .fg(*TEXT_HIGHLIGHT),
                ]))
                .italic()
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            border.inner(content_area)
        } else {
            let border = Block::bordered()
                .title_top("Playlist View")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let Some(state) = &self.props else {
            let text = "No active playlist";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg((*TEXT_NORMAL).into()))
                    .alignment(Alignment::Center),
                props.area,
            );
            return;
        };

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
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

pub struct LibraryPlaylistsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// Playlist Name Input Box
    input_box: InputBox,
    /// Is the input box visible
    input_box_visible: bool,
}

#[derive(Debug)]
pub struct Props {
    pub playlists: Box<[PlaylistBrief]>,
    sort_mode: NameSort<PlaylistBrief>,
}

impl From<&AppState> for Props {
    fn from(state: &AppState) -> Self {
        let mut playlists = state.library.playlists.clone();
        let sort_mode = NameSort::default();
        sort_mode.sort_items(&mut playlists);
        Self {
            playlists,
            sort_mode,
        }
    }
}

impl Component for LibraryPlaylistsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            input_box: InputBox::new(state, action_tx.clone()),
            input_box_visible: false,
            action_tx,
            props: Props::from(state),
            tree_state: Mutex::new(CheckTreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let tree_state = if state.active_view == ActiveView::Playlists {
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
        "Library Playlists View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        // this page has 2 distinct "modes",
        // one for navigating the tree when the input box is not visible
        // one for interacting with the input box when it is visible
        if self.input_box_visible {
            match key.code {
                // if the user presses Enter, we try to create a new playlist with the given name
                KeyCode::Enter => {
                    let name = self.input_box.text();
                    if !name.is_empty() {
                        self.action_tx
                            .send(Action::Library(LibraryAction::CreatePlaylist(
                                name.to_string(),
                            )))
                            .unwrap();
                    }
                    self.input_box.reset();
                    self.input_box_visible = false;
                }
                // defer to the input box
                _ => {
                    self.input_box.handle_key_event(key);
                }
            }
        } else {
            match key.code {
                // arrow keys
                KeyCode::PageUp => {
                    self.tree_state.lock().unwrap().select_relative(|current| {
                        current.map_or(self.props.playlists.len() - 1, |c| c.saturating_sub(10))
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
                    self.props.sort_mode.sort_items(&mut self.props.playlists);
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
                }
                KeyCode::Char('S') => {
                    self.props.sort_mode = self.props.sort_mode.prev();
                    self.props.sort_mode.sort_items(&mut self.props.playlists);
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
                }
                // "n" key to create a new playlist
                KeyCode::Char('n') => {
                    self.input_box_visible = true;
                }
                // "d" key to delete the selected playlist
                KeyCode::Char('d') => {
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::Library(LibraryAction::RemovePlaylist(thing)))
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

        if self.input_box_visible {
            let [input_box_area, content_area] = lib_split_area(area);
            let content_area = Rect {
                y: content_area.y + 1,
                height: content_area.height - 1,
                ..content_area
            };
            if input_box_area.contains(mouse_position) {
                self.input_box.handle_mouse_event(mouse, input_box_area);
            } else if content_area.contains(mouse_position)
                && kind == MouseEventKind::Down(MouseButton::Left)
            {
                self.input_box_visible = false;
            }
        } else {
            let area = Rect {
                y: area.y + 1,
                height: area.height - 1,
                ..area
            };

            let result = self
                .tree_state
                .lock()
                .unwrap()
                .handle_mouse_event(mouse, area, true);
            if let Some(action) = result {
                self.action_tx.send(action).unwrap();
            }
        }
    }
}

fn lib_split_area(area: Rect) -> [Rect; 2] {
    let [input_box_area, content_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area)
    else {
        panic!("Failed to split library playlists view area");
    };
    [input_box_area, content_area]
}

impl ComponentRender<RenderProps> for LibraryPlaylistsView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        // render primary border
        let border_title_bottom = if self.input_box_visible {
            ""
        } else {
            " \u{23CE} : Open | ←/↑/↓/→: Navigate | s/S: change sort"
        };
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Playlists".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(border_title_bottom)
            .border_style(border_style);
        let content_area = border.inner(props.area);
        frame.render_widget(border, props.area);

        // render input box (if visible)
        let content_area = if self.input_box_visible {
            // split content area to make room for the input box
            let [input_box_area, content_area] = lib_split_area(content_area);

            // render the input box
            self.input_box.render(
                frame,
                input_box::RenderProps {
                    area: input_box_area,
                    text_color: (*TEXT_HIGHLIGHT_ALT).into(),
                    border: Block::bordered()
                        .title("Enter Name:")
                        .border_style(Style::default().fg((*BORDER_FOCUSED).into())),
                    show_cursor: self.input_box_visible,
                },
            );

            content_area
        } else {
            content_area
        };

        // draw additional border around content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP)
            .title_top(if self.input_box_visible {
                " \u{23CE} : Create (cancel if empty)"
            } else {
                "n: new playlist | d: delete playlist"
            })
            .border_style(border_style);
        let area = border.inner(content_area);
        frame.render_widget(border, content_area);

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        // create a tree for the playlists
        let items = self
            .props
            .playlists
            .iter()
            .map(create_playlist_tree_leaf)
            .collect::<Vec<_>>();

        // render the playlists
        frame.render_stateful_widget(
            CheckTree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                // we want this to be rendered like a normal tree, not a check tree, so we don't show the checkboxes
                .node_unchecked_symbol("▪ ")
                .node_checked_symbol("▪ ")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

#[cfg(test)]
mod sort_mode_tests {
    use super::*;
    use mecomp_storage::db::schemas::playlist::Playlist;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(NameSort::default(), NameSort::default())]
    fn test_sort_mode_next_prev(
        #[case] mode: NameSort<PlaylistBrief>,
        #[case] expected: NameSort<PlaylistBrief>,
    ) {
        assert_eq!(mode.next(), expected);
        assert_eq!(mode.next().prev(), mode);
    }

    #[rstest]
    #[case(NameSort::default(), "Name")]
    fn test_sort_mode_display(#[case] mode: NameSort<PlaylistBrief>, #[case] expected: &str) {
        assert_eq!(mode.to_string(), expected);
    }

    #[rstest]
    fn test_sort_items() {
        let mut songs = vec![
            PlaylistBrief {
                id: Playlist::generate_id(),
                name: "C".into(),
            },
            PlaylistBrief {
                id: Playlist::generate_id(),
                name: "A".into(),
            },
            PlaylistBrief {
                id: Playlist::generate_id(),
                name: "B".into(),
            },
        ];

        NameSort::default().sort_items(&mut songs);
        assert_eq!(songs[0].name, "A");
        assert_eq!(songs[1].name, "B");
        assert_eq!(songs[2].name, "C");
    }
}

#[cfg(test)]
mod item_view_tests {
    use super::*;
    use crate::{
        state::action::{AudioAction, PopupAction, QueueAction},
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::{components::content_view::ActiveView, widgets::popups::PopupType},
    };
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = PlaylistView::new(&state, tx);

        assert_eq!(view.name(), "Playlist View");
        assert_eq!(
            view.props,
            Some(state.additional_view_data.playlist.unwrap())
        );
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = PlaylistView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(
            view.props,
            Some(new_state.additional_view_data.playlist.unwrap())
        );
    }
    #[test]
    fn test_render_no_playlist() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = PlaylistView::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(20, 3);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌Playlist View─────┐",
            "│No active playlist│",
            "└──────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = PlaylistView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire playlist──────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_checked() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = PlaylistView::new(&state_with_everything(), tx);
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire playlist──────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // select the song
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│☑ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn smoke_navigation_and_sort() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = PlaylistView::new(&state_with_everything(), tx);

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        view.handle_key_event(KeyEvent::from(KeyCode::PageUp));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
        view.handle_key_event(KeyEvent::from(KeyCode::Left));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
    }

    #[test]
    fn test_actions() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = PlaylistView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // we test the actions when:
        // there are no checked items
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("playlist", item_id()).into()
            ])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("playlist", item_id()).into()
            ],)))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("playlist", item_id()).into()
            ])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));

        // there are checked items
        // first we need to select an item
        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
        );

        // check the artist
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("song", item_id()).into()
            ])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("song", item_id()).into()
            ],)))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("song", item_id()).into()
            ])))
        );

        // remove from playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::RemoveSongsFromPlaylist(
                ("playlist", item_id()).into(),
                vec![("song", item_id()).into()]
            ))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_mouse_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = PlaylistView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire playlist──────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click on the song (selecting it)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 6,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│☑ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click down the song (unselecting it)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 6,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let expected = Buffer::with_lines([
            "┌Playlist View sorted by: Artist───────────────────────────┐",
            "│                       Test Playlist                      │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire playlist──────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: sort | d: remove selected | e: edit──────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);
        // ctrl-click on the song (opening it)
        for _ in 0..2 {
            view.handle_mouse_event(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: 2,
                    row: 6,
                    modifiers: KeyModifiers::CONTROL,
                },
                area,
            );
            assert_eq!(
                rx.blocking_recv().unwrap(),
                Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
            );
        }

        // scroll down
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 6,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);
        // scroll up
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 2,
                row: 7,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);
    }
}

#[cfg(test)]
mod library_view_tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = LibraryPlaylistsView::new(&state, tx);

        assert_eq!(view.name(), "Library Playlists View");
        assert_eq!(view.props.playlists, state.library.playlists);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = LibraryPlaylistsView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(view.props.playlists, new_state.library.playlists);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(60, 6);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Playlists sorted by: Name─────────────────────────┐",
            "│n: new playlist | d: delete playlist──────────────────────│",
            "│▪ Test Playlist                                           │",
            "│                                                          │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | s/S: change sort──────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_input_box() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        // open the input box
        view.handle_key_event(KeyEvent::from(KeyCode::Char('n')));

        let (mut terminal, area) = setup_test_terminal(60, 7);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Playlists sorted by: Name─────────────────────────┐",
            "│┌Enter Name:─────────────────────────────────────────────┐│",
            "││                                                        ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│ ⏎ : Create (cancel if empty)─────────────────────────────│",
            "│▪ Test Playlist                                           │",
            "└──────────────────────────────────────────────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, NameSort::default());
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        view.handle_key_event(KeyEvent::from(KeyCode::PageUp));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
        view.handle_key_event(KeyEvent::from(KeyCode::Left));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
    }

    #[test]
    fn test_actions() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        terminal.draw(|frame| view.render(frame, props)).unwrap();

        // first we need to navigate to the playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Playlist(item_id())))
        );

        // new playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(view.input_box_visible, true);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('b')));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(view.input_box_visible, false);
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::CreatePlaylist("ab".to_string()))
        );

        // delete playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::RemovePlaylist(
                ("playlist", item_id()).into()
            ))
        );
    }

    #[test]
    fn test_mouse_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryPlaylistsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Playlists sorted by: Name─────────────────────────┐",
            "│n: new playlist | d: delete playlist──────────────────────│",
            "│▪ Test Playlist                                           │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | s/S: change sort──────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click on the playlist when it's not selected
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Playlist(item_id())))
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);

        // scroll down (selecting the collection)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // click down the playlist when it is selected
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Playlist(item_id())))
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);

        // scroll up
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 2,
                row: 3,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // click down on selected item
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Playlist(item_id())))
        );

        // clicking on an empty area should clear the selection
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 3,
            modifiers: KeyModifiers::empty(),
        };
        view.handle_mouse_event(mouse, area);
        assert_eq!(view.tree_state.lock().unwrap().get_selected_thing(), None);
        view.handle_mouse_event(mouse, area);
        assert_eq!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        );
    }
}
