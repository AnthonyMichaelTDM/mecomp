//! Views for both a single album, and the library of albums.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::{album::Album, Thing};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT},
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::popups::PopupType,
        AppState,
    },
};

use super::{
    none::NoneView,
    utils::{create_album_tree_leaf, create_artist_tree_item, create_song_tree_item},
    AlbumViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct AlbumView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<AlbumViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

impl Component for AlbumView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.album.clone(),
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.album {
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
            // Add album to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            // Start radio from album
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
            // add album to playlist
            KeyCode::Char('p') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for AlbumView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top("Song View")
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold album artists and songs
            let artist_tree = create_artist_tree_item(state.artists.as_slice()).unwrap();
            let song_tree = create_song_tree_item(&state.songs).unwrap();
            let items = &[artist_tree, song_tree];

            let [top, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(block_area)
            else {
                panic!("Failed to split song view area")
            };

            // render the album info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(state.album.title.to_string(), Style::default().bold()),
                        Span::raw(" "),
                        Span::styled(
                            state
                                .album
                                .artist
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<String>>()
                                .join(", "),
                            Style::default().italic(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Release Year: "),
                        Span::styled(
                            state
                                .album
                                .release
                                .map_or_else(|| "unknown".to_string(), |y| y.to_string()),
                            Style::default().italic(),
                        ),
                        Span::raw("  Songs: "),
                        Span::styled(
                            state.album.song_count.to_string(),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format!(
                                "{}:{:04.1}",
                                state.album.runtime.as_secs() / 60,
                                state.album.runtime.as_secs_f32() % 60.0,
                            ),
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
                    .highlight_style(
                        Style::default()
                            .fg(TEXT_HIGHLIGHT.into())
                            .add_modifier(Modifier::BOLD),
                    )
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

pub struct LibraryAlbumsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

struct Props {
    albums: Box<[Album]>,
    sort_mode: SortMode,
}

#[derive(Default)]
pub enum SortMode {
    Title,
    #[default]
    Artist,
    ReleaseYear,
}

impl Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Title => write!(f, "Title"),
            Self::Artist => write!(f, "Artist"),
            Self::ReleaseYear => write!(f, "Year"),
        }
    }
}

impl SortMode {
    pub const fn next(&self) -> Self {
        match self {
            Self::Title => Self::Artist,
            Self::Artist => Self::ReleaseYear,
            Self::ReleaseYear => Self::Title,
        }
    }

    pub const fn prev(&self) -> Self {
        match self {
            Self::Title => Self::ReleaseYear,
            Self::Artist => Self::Title,
            Self::ReleaseYear => Self::Artist,
        }
    }

    pub fn sort_albums(&self, albums: &mut [Album]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }

        match self {
            Self::Title => albums.sort_by_key(|album| key(&album.title)),
            Self::Artist => {
                albums.sort_by_cached_key(|album| album.artist.iter().map(key).collect::<Vec<_>>());
            }
            Self::ReleaseYear => {
                albums.sort_by_key(|album| album.release.unwrap_or(0));
                albums.reverse();
            }
        }
    }
}

impl Component for LibraryAlbumsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = SortMode::default();
        let mut albums = state.library.albums.clone();
        sort_mode.sort_albums(&mut albums);
        Self {
            action_tx,
            props: Props { albums, sort_mode },
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut albums = state.library.albums.clone();
        self.props.sort_mode.sort_albums(&mut albums);
        Self {
            props: Props {
                albums,
                ..self.props
            },
            ..self
        }
    }

    fn name(&self) -> &str {
        "Library Albums View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.albums.len() - 1, |c| c.saturating_sub(10))
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
                self.props.sort_mode.sort_albums(&mut self.props.albums);
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_albums(&mut self.props.albums);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for LibraryAlbumsView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let block = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Albums".to_string(), Style::default().bold()),
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
            panic!("Failed to split library albums view area");
        };

        let items = self
            .props
            .albums
            .iter()
            .map(|album| create_album_tree_leaf(album, None))
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
