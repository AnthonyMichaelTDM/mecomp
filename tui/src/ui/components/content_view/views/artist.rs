//! Views for both a single artist, and the library of artists.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::{artist::Artist, Thing};
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
    utils::{create_album_tree_item, create_artist_tree_leaf, create_song_tree_item},
    ArtistViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct ArtistView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<ArtistViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

impl Component for ArtistView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.artist.clone(),
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.artist {
            Self {
                props: Some(props.to_owned()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Artist View"
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
            // Add artist to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            // Start radio from artist
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
            // add artist to playlist
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

impl ComponentRender<RenderProps> for ArtistView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top("Artist View")
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold artist albums and songs
            let album_tree = create_album_tree_item(&state.albums).unwrap();
            let song_tree = create_song_tree_item(&state.songs).unwrap();
            let items = &[album_tree, song_tree];

            let [top, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(block_area)
            else {
                panic!("Failed to split artist view area")
            };

            // render the artist info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        state.artist.name.to_string(),
                        Style::default().bold(),
                    )),
                    Line::from(vec![
                        Span::raw("Albums: "),
                        Span::styled(
                            state.artist.album_count.to_string(),
                            Style::default().italic(),
                        ),
                        Span::raw("  Songs: "),
                        Span::styled(
                            state.artist.song_count.to_string(),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format!(
                                "{}:{:04.1}",
                                state.artist.runtime.as_secs() / 60,
                                state.artist.runtime.as_secs_f32() % 60.0,
                            ),
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

            // render the artist artists / album
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

pub struct LibraryArtistsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

struct Props {
    artists: Box<[Artist]>,
    sort_mode: SortMode,
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
    pub fn sort_artists(&self, artists: &mut [Artist]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        artists.sort_by_key(|artist| key(&artist.name));
    }
}

impl Component for LibraryArtistsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = SortMode::default();
        let mut artists = state.library.artists.clone();
        sort_mode.sort_artists(&mut artists);
        Self {
            action_tx,
            props: Props { artists, sort_mode },
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut artists = state.library.artists.clone();
        self.props.sort_mode.sort_artists(&mut artists);
        Self {
            props: Props {
                artists,
                ..self.props
            },
            ..self
        }
    }

    fn name(&self) -> &str {
        "Library Artists View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.artists.len() - 1, |c| c.saturating_sub(10))
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
                self.props.sort_mode.sort_artists(&mut self.props.artists);
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_artists(&mut self.props.artists);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for LibraryArtistsView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let block = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Artists".to_string(), Style::default().bold()),
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
            panic!("Failed to split library artists view area");
        };

        let items = self
            .props
            .artists
            .iter()
            .map(|artist| create_artist_tree_leaf(artist))
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
