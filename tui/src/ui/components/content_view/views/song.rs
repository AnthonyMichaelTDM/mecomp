//! Views for both a single song, and the library of songs.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::song::Song;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_NORMAL},
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::{
            popups::PopupType,
            tree::{state::CheckTreeState, CheckTree},
        },
        AppState,
    },
};

use super::{
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        construct_start_radio_action, create_album_tree_leaf, create_artist_tree_item,
        create_song_tree_leaf, get_checked_things_from_tree_state,
        get_selected_things_from_tree_state,
    },
    SongViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct SongView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<SongViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

impl Component for SongView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.song.clone(),
            tree_state: Mutex::new(CheckTreeState::default()),
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
            KeyCode::Char(' ') => {
                self.tree_state.lock().unwrap().key_space();
            }
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, add them to the queue, otherwise send the song to the queue
            KeyCode::Char('q') => {
                if let Some(action) = construct_add_to_queue_action(
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()),
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, start radio from checked items, otherwise start radio from song
            KeyCode::Char('r') => {
                if let Some(action) = construct_start_radio_action(
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()),
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to playlist, otherwise add the song to playlist
            KeyCode::Char('p') => {
                if let Some(action) = construct_add_to_playlist_action(
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()),
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for SongView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let border = Block::bordered()
            .title_top("Song View")
            .border_style(border_style);
        frame.render_widget(&border, props.area);
        // draw borders and get area for the content (album and artists of song)
        let area = self.props.as_ref().map_or_else(
            || border.inner(props.area),
            |state| {
                let area = border.inner(props.area);

                // split area to make room for song info
                let [info_area, content_area] = *Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(4)])
                    .split(area)
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
                    .alignment(Alignment::Center),
                    info_area,
                );

                // draw an additional border around the content area to display additional instructions
                let border = Block::new()
                    .borders(Borders::TOP)
                    .title_top("q: add to queue | r: start radio | p: add to playlist")
                    .border_style(border_style);
                frame.render_widget(&border, content_area);
                let content_area = border.inner(content_area);

                // draw an additional border around the content area to indicate whether operations will be performed on the entire item, or just the checked items
                let border = Block::default()
                    .borders(Borders::TOP)
                    .title_top(Line::from(vec![
                        Span::raw("Performing operations on "),
                        Span::raw(
                            if get_checked_things_from_tree_state(&self.tree_state.lock().unwrap())
                                .is_empty()
                            {
                                "the song"
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
            },
        );

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create a tree to hold song album and artists
            let album_tree = create_album_tree_leaf(&state.album, Some(Span::raw("Album: ")));
            let artist_tree = create_artist_tree_item(state.artists.as_slice()).unwrap();
            let items = &[artist_tree, album_tree];

            // render the song artists / album tree
            frame.render_stateful_widget(
                CheckTree::new(items)
                    .unwrap()
                    .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold()),
                props.area,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            let text = "No active song";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}

pub struct LibrarySongsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub(crate) props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

pub(crate) struct Props {
    pub(crate) songs: Box<[Song]>,
    pub(crate) sort_mode: SortMode,
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
    #[must_use]
    pub const fn next(&self) -> Self {
        match self {
            Self::Title => Self::Artist,
            Self::Artist => Self::Album,
            Self::Album => Self::AlbumArtist,
            Self::AlbumArtist => Self::Genre,
            Self::Genre => Self::Title,
        }
    }

    #[must_use]
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
            tree_state: Mutex::new(CheckTreeState::default()),
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
            KeyCode::Char(' ') => {
                self.tree_state.lock().unwrap().key_space();
            }
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // when there are checked items, "q" will send the checked items to the queue
            KeyCode::Char('q') => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
                        .unwrap();
                }
            }
            // when there are checked items, "r" will start a radio with the checked items
            KeyCode::Char('r') => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::SetCurrentView(ActiveView::Radio(
                            things, RADIO_SIZE,
                        )))
                        .unwrap();
                }
            }
            // when there are checked items, "p" will send the checked items to the playlist
            KeyCode::Char('p') => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(
                            things,
                        ))))
                        .unwrap();
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
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // draw primary border
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Songs".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
            .border_style(border_style);
        let content_area = border.inner(props.area);
        frame.render_widget(border, props.area);

        // draw an additional border around the content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .title_top(
                if get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()).is_empty() {
                    ""
                } else {
                    "q: add to queue | r: start radio | p: add to playlist "
                },
            )
            .title_bottom("s/S: change sort")
            .border_style(border_style);
        frame.render_widget(&border, content_area);
        let content_area = border.inner(content_area);

        RenderProps {
            area: content_area,
            is_focused: props.is_focused,
        }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        // create a tree to hold the songs
        let items = self
            .props
            .songs
            .iter()
            .map(|song| create_song_tree_leaf(song))
            .collect::<Vec<_>>();

        // render the tree
        frame.render_stateful_widget(
            CheckTree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
