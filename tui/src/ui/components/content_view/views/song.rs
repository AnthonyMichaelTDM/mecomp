//! Views for both a single song, and the library of songs.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::{song::Song, Thing};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::{Action, AudioAction, QueueAction},
    ui::{
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        AppState,
    },
};

use super::{
    none::NoneView,
    utils::{create_album_tree_leaf, create_artist_tree_item, create_song_tree_leaf},
    SongViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct SongView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<SongViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

impl Component for SongView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.song.clone(),
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.song {
            Self {
                props: Some(props.to_owned()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Song View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::Up => {
                self.tree_state.lock().unwrap().key_up();
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
            // Add song to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            // Start radio from song
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
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for SongView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top("Song View")
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold song album and artists
            let album_tree = create_album_tree_leaf(&state.album, Some(Span::raw("Album: ")));
            let artist_tree = create_artist_tree_item(state.artists.as_slice()).unwrap();
            let items = &[artist_tree, album_tree];

            let [top, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(block_area)
            else {
                panic!("Failed to split song view area")
            };

            // render the song info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(state.song.title.to_string(), Style::default().bold()),
                        Span::raw(" "),
                        Span::styled(
                            state
                                .song
                                .artist
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<String>>()
                                .join(", "),
                            Style::default().italic(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Track/Disc: "),
                        Span::styled(
                            format!(
                                "{}/{}",
                                state.song.track.unwrap_or_default(),
                                state.song.disc.unwrap_or_default()
                            ),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format!(
                                "{}:{:04.1}",
                                state.song.runtime.as_secs() / 60,
                                state.song.runtime.as_secs_f32() % 60.0,
                            ),
                            Style::default().italic(),
                        ),
                        Span::raw("  Genre(s): "),
                        Span::styled(
                            state
                                .song
                                .genre
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<String>>()
                                .join(", "),
                            Style::default().italic(),
                        ),
                    ]),
                ])
                .block(
                    Block::new()
                        .borders(Borders::BOTTOM)
                        .title_bottom("q: add to queue | r: start radio")
                        .border_style(border_style),
                )
                .alignment(Alignment::Center),
                top,
            );

            // render the song artists / album
            frame.render_stateful_widget(
                Tree::new(items)
                    .unwrap()
                    .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    .node_closed_symbol("▸")
                    .node_open_symbol("▾")
                    .node_no_children_symbol("▪"),
                bottom,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            NoneView.render(frame, props);
        }
    }
}

pub struct LibrarySongsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

struct Props {
    songs: Box<[Song]>,
    sort_mode: SortMode,
}

#[derive(Default)]
pub enum SortMode {
    Title,
    #[default]
    Artist,
    Album,
    AlbumArtist,
    Genre,
}

impl Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Title => write!(f, "Title"),
            Self::Artist => write!(f, "Artist"),
            Self::Album => write!(f, "Album"),
            Self::AlbumArtist => write!(f, "Album Artist"),
            Self::Genre => write!(f, "Genre"),
        }
    }
}

impl SortMode {
    pub const fn next(&self) -> Self {
        match self {
            Self::Title => Self::Artist,
            Self::Artist => Self::Album,
            Self::Album => Self::AlbumArtist,
            Self::AlbumArtist => Self::Genre,
            Self::Genre => Self::Title,
        }
    }

    pub const fn prev(&self) -> Self {
        match self {
            Self::Title => Self::Genre,
            Self::Artist => Self::Title,
            Self::Album => Self::Artist,
            Self::AlbumArtist => Self::Album,
            Self::Genre => Self::AlbumArtist,
        }
    }

    pub fn sort_songs(&self, songs: &mut [Song]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }

        match self {
            Self::Title => songs.sort_by_key(|song| key(&song.title)),
            Self::Artist => {
                songs.sort_by_cached_key(|song| song.artist.iter().map(key).collect::<Vec<_>>());
            }
            Self::Album => songs.sort_by_key(|song| key(&song.album)),
            Self::AlbumArtist => {
                songs.sort_by_cached_key(|song| {
                    song.album_artist.iter().map(key).collect::<Vec<_>>()
                });
            }
            Self::Genre => {
                songs.sort_by_cached_key(|song| song.genre.iter().map(key).collect::<Vec<_>>());
            }
        }
    }
}

impl Component for LibrarySongsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = SortMode::default();
        let mut songs = state.library.songs.clone();
        sort_mode.sort_songs(&mut songs);
        Self {
            action_tx,
            props: Props { songs, sort_mode },
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut songs = state.library.songs.clone();
        self.props.sort_mode.sort_songs(&mut songs);
        Self {
            props: Props {
                songs,
                ..self.props
            },
            ..self
        }
    }

    fn name(&self) -> &str {
        "Library Songs View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.songs.len() - 1, |c| c.saturating_sub(10))
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
                self.props.sort_mode.sort_songs(&mut self.props.songs);
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_songs(&mut self.props.songs);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for LibrarySongsView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        let block = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Songs".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom("Enter: Open | ←/↑/↓/→: Navigate | s/S: change sort")
            .border_style(border_style);
        let block_area = block.inner(props.area);
        frame.render_widget(block, props.area);

        let [top, bottom] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(block_area)
        else {
            panic!("Failed to split library songs view area");
        };

        let items = self
            .props
            .songs
            .iter()
            .map(|song| create_song_tree_leaf(song))
            .collect::<Vec<_>>();

        frame.render_widget(
            Block::new()
                .borders(Borders::BOTTOM)
                .border_style(border_style),
            top,
        );

        frame.render_stateful_widget(
            Tree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .node_closed_symbol("▸")
                .node_open_symbol("▾")
                .node_no_children_symbol("▪")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            bottom,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
