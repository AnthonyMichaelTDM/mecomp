//! Views for both a single playlist, and the library of playlists.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::{playlist::Playlist, Thing};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::{Action, AudioAction, LibraryAction, PopupAction, QueueAction},
    ui::{
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
        },
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::{
            input_box::{self, InputBox},
            popups::PopupType,
        },
        AppState,
    },
};

use super::{
    none::NoneView,
    utils::{create_playlist_tree_leaf, create_song_tree_leaf},
    PlaylistViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct PlaylistView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<PlaylistViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
    /// sort mode
    sort_mode: super::song::SortMode,
}

impl Component for PlaylistView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.playlist.clone(),
            tree_state: Mutex::new(TreeState::default()),
            sort_mode: super::song::SortMode::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.playlist {
            let mut props = props.clone();
            self.sort_mode.sort_songs(&mut props.songs);

            Self {
                props: Some(props),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Playlist View"
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
            // Change sort mode
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.next();
                if let Some(props) = &mut self.props {
                    self.sort_mode.sort_songs(&mut props.songs);
                }
            }
            KeyCode::Char('S') => {
                self.sort_mode = self.sort_mode.prev();
                if let Some(props) = &mut self.props {
                    self.sort_mode.sort_songs(&mut props.songs);
                }
            }
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things: Vec<Thing> = self
                        .tree_state
                        .lock()
                        .unwrap()
                        .selected()
                        .iter()
                        .filter_map(|id| id.parse::<Thing>().ok())
                        .collect();
                    if !things.is_empty() {
                        debug_assert!(things.len() == 1);
                        let thing = things[0].clone();
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // Add playlist to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            // Start radio from playlist
            KeyCode::Char('r') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::SetCurrentView(ActiveView::Radio(
                            vec![props.id.clone()],
                            RADIO_SIZE,
                        )))
                        .unwrap();
                }
            }
            // add playlist to playlist
            KeyCode::Char('p') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            // Delete selected song
            KeyCode::Char('d') => {
                if let Some(props) = &self.props {
                    let things: Vec<Thing> = self
                        .tree_state
                        .lock()
                        .unwrap()
                        .selected()
                        .iter()
                        .filter_map(|id| id.parse::<Thing>().ok())
                        .collect();
                    if !things.is_empty() {
                        debug_assert!(things.len() == 1);
                        self.action_tx
                            .send(Action::Library(LibraryAction::RemoveSongsFromPlaylist(
                                props.id.clone(),
                                things,
                            )))
                            .unwrap();
                    }
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for PlaylistView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Playlist View".to_string(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold playlist songs
            let items = state
                .songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect::<Vec<_>>();

            let [top, middle, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(1),
                ])
                .split(block_area)
            else {
                panic!("Failed to split playlist view area")
            };

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
                .block(
                    Block::new()
                        .borders(Borders::BOTTOM)
                        .title_bottom("q: add to queue | r: start radio | p: add to playlist")
                        .border_style(border_style),
                )
                .alignment(Alignment::Center),
                top,
            );

            // render the playlist songs
            frame.render_stateful_widget(
                Tree::new(&items)
                    .unwrap()
                    .highlight_style(
                        Style::default()
                            .fg(TEXT_HIGHLIGHT.into())
                            .add_modifier(Modifier::BOLD),
                    )
                    .node_closed_symbol("▸")
                    .node_open_symbol("▾")
                    .node_no_children_symbol("▪")
                    .experimental_scrollbar(Some(Scrollbar::new(
                        ScrollbarOrientation::VerticalRight,
                    ))),
                middle,
                &mut self.tree_state.lock().unwrap(),
            );

            // render the instructions
            frame.render_widget(
                Block::new()
                    .borders(Borders::TOP)
                    .title_top("s/S: change sort | d: remove selected song")
                    .border_style(border_style),
                bottom,
            );
        } else {
            NoneView.render(frame, props);
        }
    }
}

pub struct LibraryPlaylistsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
    /// Playlist Name Input Box
    input_box: InputBox,
    /// Is the input box visible
    input_box_visible: bool,
}

pub struct Props {
    pub playlists: Box<[Playlist]>,
    sort_mode: SortMode,
}

impl From<&AppState> for Props {
    fn from(state: &AppState) -> Self {
        let mut playlists = state.library.playlists.clone();
        let sort_mode = SortMode::default();
        sort_mode.sort_playlists(&mut playlists);
        Self {
            playlists,
            sort_mode,
        }
    }
}

#[derive(Default)]
pub enum SortMode {
    #[default]
    Name,
}

impl Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name => write!(f, "Name"),
        }
    }
}

impl SortMode {
    pub const fn next(&self) -> Self {
        match self {
            Self::Name => Self::Name,
        }
    }

    pub const fn prev(&self) -> Self {
        match self {
            Self::Name => Self::Name,
        }
    }

    #[allow(clippy::unused_self)]
    pub fn sort_playlists(&self, playlists: &mut [Playlist]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        playlists.sort_by_key(|playlist| key(&playlist.name));
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
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: Props::from(state),
            ..self
        }
    }

    fn name(&self) -> &str {
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
                        let things: Vec<Thing> = self
                            .tree_state
                            .lock()
                            .unwrap()
                            .selected()
                            .iter()
                            .filter_map(|id| id.parse::<Thing>().ok())
                            .collect();
                        if !things.is_empty() {
                            debug_assert!(things.len() == 1);
                            let thing = things[0].clone();
                            self.action_tx
                                .send(Action::SetCurrentView(thing.into()))
                                .unwrap();
                        }
                    }
                }
                // Change sort mode
                KeyCode::Char('s') => {
                    self.props.sort_mode = self.props.sort_mode.next();
                    self.props
                        .sort_mode
                        .sort_playlists(&mut self.props.playlists);
                }
                KeyCode::Char('S') => {
                    self.props.sort_mode = self.props.sort_mode.prev();
                    self.props
                        .sort_mode
                        .sort_playlists(&mut self.props.playlists);
                }
                // "n" key to create a new playlist
                KeyCode::Char('n') => {
                    self.input_box_visible = true;
                }
                // "d" key to delete the selected playlist
                KeyCode::Char('d') => {
                    let things: Vec<Thing> = self
                        .tree_state
                        .lock()
                        .unwrap()
                        .selected()
                        .iter()
                        .filter_map(|id| id.parse::<Thing>().ok())
                        .collect();
                    if !things.is_empty() {
                        debug_assert!(things.len() == 1);
                        let thing = things[0].clone();
                        self.action_tx
                            .send(Action::Library(LibraryAction::RemovePlaylist(thing)))
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
    }
}

impl ComponentRender<RenderProps> for LibraryPlaylistsView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let block = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Playlists".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(if self.input_box_visible {
                ""
            } else {
                "Enter: Open | ←/↑/↓/→: Navigate | s/S: change sort"
            })
            .border_style(border_style);
        let block_area = block.inner(props.area);
        frame.render_widget(block, props.area);

        let [top, middle, bottom] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if self.input_box_visible { 3 } else { 0 }),
                Constraint::Length(1),
                Constraint::Min(4),
            ])
            .split(block_area)
        else {
            panic!("Failed to split library playlists view area");
        };

        let items = self
            .props
            .playlists
            .iter()
            .map(|playlist| create_playlist_tree_leaf(playlist))
            .collect::<Vec<_>>();

        // render input box
        if self.input_box_visible {
            self.input_box.render(
                frame,
                input_box::RenderProps {
                    area: top,
                    text_color: if self.input_box_visible {
                        TEXT_HIGHLIGHT_ALT.into()
                    } else {
                        TEXT_NORMAL.into()
                    },
                    border: Block::bordered().title("Enter Name:").border_style(
                        Style::default().fg(if self.input_box_visible && props.is_focused {
                            BORDER_FOCUSED.into()
                        } else {
                            BORDER_UNFOCUSED.into()
                        }),
                    ),
                    show_cursor: self.input_box_visible,
                },
            );
        }

        // render instruction bar
        frame.render_widget(
            Block::new()
                .borders(Borders::BOTTOM)
                .title_bottom(if self.input_box_visible {
                    "Enter: Create (cancel if empty)"
                } else {
                    "n: new playlist | d: delete playlist"
                })
                .border_style(border_style),
            middle,
        );

        // render playlist list
        frame.render_stateful_widget(
            Tree::new(&items)
                .unwrap()
                .highlight_style(
                    Style::default()
                        .fg(TEXT_HIGHLIGHT.into())
                        .add_modifier(Modifier::BOLD),
                )
                .node_closed_symbol("▸")
                .node_open_symbol("▾")
                .node_no_children_symbol("▪")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            bottom,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
