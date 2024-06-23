//! Views for both a single collection, and the library of collections.

// TODO: button to freeze the collection into a new playlist

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::{collection::Collection, Thing};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::{Action, AudioAction, QueueAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT},
        components::{Component, ComponentRender, RenderProps},
        AppState,
    },
};

use super::{
    none::NoneView,
    utils::{create_collection_tree_leaf, create_song_tree_leaf},
    CollectionViewProps,
};

#[allow(clippy::module_name_repetitions)]
pub struct CollectionView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<CollectionViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
    /// sort mode
    sort_mode: super::song::SortMode,
}

impl Component for CollectionView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.collection.clone(),
            tree_state: Mutex::new(TreeState::default()),
            sort_mode: super::song::SortMode::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.collection {
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
        "Collection View"
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
            // Add collection to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for CollectionView {
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
                    Span::styled("Collection View".to_string(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold collection songs
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
                panic!("Failed to split collection view area")
            };

            // render the collection info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        state.collection.name.to_string(),
                        Style::default().bold(),
                    )),
                    Line::from(vec![
                        Span::raw("Songs: "),
                        Span::styled(
                            state.collection.song_count.to_string(),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format_duration(&state.collection.runtime),
                            Style::default().italic(),
                        ),
                    ]),
                ])
                .block(
                    Block::new()
                        .borders(Borders::BOTTOM)
                        .title_bottom("q: add to queue")
                        .border_style(border_style),
                )
                .alignment(Alignment::Center),
                top,
            );

            // render the collections songs
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

pub struct LibraryCollectionsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

struct Props {
    collections: Box<[Collection]>,
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
    pub fn sort_collections(&self, collections: &mut [Collection]) {
        fn key<T: AsRef<str>>(input: T) -> String {
            input
                .as_ref()
                .to_lowercase() // ignore case
                .trim_start_matches(|c: char| !c.is_alphanumeric()) // ignore leading non-alphanumeric characters
                .trim_start_matches("the ") // ignore leading "the "
                .to_owned()
        }
        collections.sort_by_key(|collection| key(&collection.name));
    }
}

impl Component for LibraryCollectionsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = SortMode::default();
        let mut collections = state.library.collections.clone();
        sort_mode.sort_collections(&mut collections);
        Self {
            action_tx,
            props: Props {
                collections,
                sort_mode,
            },
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut collections = state.library.collections.clone();
        self.props.sort_mode.sort_collections(&mut collections);
        Self {
            props: Props {
                collections,
                ..self.props
            },
            ..self
        }
    }

    fn name(&self) -> &str {
        "Library Collections View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.collections.len() - 1, |c| c.saturating_sub(10))
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
                    .sort_collections(&mut self.props.collections);
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props
                    .sort_mode
                    .sort_collections(&mut self.props.collections);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for LibraryCollectionsView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let block = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Collections".to_string(), Style::default().bold()),
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
            panic!("Failed to split library collections view area");
        };

        let items = self
            .props
            .collections
            .iter()
            .map(|collection| create_collection_tree_leaf(collection))
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
