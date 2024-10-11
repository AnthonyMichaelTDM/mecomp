//! Views for both a single collection, and the library of collections.

// TODO: button to freeze the collection into a new playlist

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::collection::Collection;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::Action,
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_NORMAL},
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::tree::{state::CheckTreeState, CheckTree},
        AppState,
    },
};

use super::{
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        create_collection_tree_leaf, create_song_tree_leaf, get_checked_things_from_tree_state,
        get_selected_things_from_tree_state,
    },
    CollectionViewProps,
};

#[allow(clippy::module_name_repetitions)]
pub struct CollectionView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<CollectionViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
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
            tree_state: Mutex::new(CheckTreeState::default()),
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
                tree_state: Mutex::new(CheckTreeState::default()),
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
            KeyCode::Char(' ') => {
                self.tree_state.lock().unwrap().key_space();
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
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, add them to the queue, otherwise send the whole collection to the queue
            KeyCode::Char('q') => {
                let checked_things =
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if let Some(action) = construct_add_to_queue_action(
                    checked_things,
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to the playlist, otherwise send the whole collection to the playlist
            KeyCode::Char('p') => {
                let checked_things =
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if let Some(action) = construct_add_to_playlist_action(
                    checked_things,
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));
        let [_, content_area] = split_area(area);
        let content_area = content_area.inner(Margin::new(0, 1));

        match kind {
            MouseEventKind::Down(MouseButton::Left) if content_area.contains(mouse_position) => {
                let selected_things =
                    get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());
                self.tree_state.lock().unwrap().mouse_click(mouse_position);

                // if the selection didn't change, open the selected view
                if selected_things
                    == get_selected_things_from_tree_state(&self.tree_state.lock().unwrap())
                {
                    if let Some(thing) = selected_things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            MouseEventKind::ScrollDown if content_area.contains(mouse_position) => {
                self.tree_state.lock().unwrap().scroll_down(1);
            }
            MouseEventKind::ScrollUp if content_area.contains(mouse_position) => {
                self.tree_state.lock().unwrap().scroll_up(1);
            }
            _ => {}
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [info_area, content_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area)
    else {
        panic!("Failed to split collection view area")
    };

    [info_area, content_area]
}

impl ComponentRender<RenderProps> for CollectionView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Collection View".to_string(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            let content_area = border.inner(props.area);

            // split content area to make room for the collection info
            let [info_area, content_area] = split_area(content_area);

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
                .alignment(Alignment::Center),
                info_area,
            );

            // draw an additional border around the content area to display additionally instructions
            let border = Block::new()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title_top("q: add to queue | p: add to playlist")
                .title_bottom("s/S: change sort")
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
                            "entire collection"
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
                .title_top("Collection View")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create list to hold collection songs
            let items = state
                .songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect::<Vec<_>>();

            // render the collections songs
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
            let text = "No active collection";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}

pub struct LibraryCollectionsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

struct Props {
    collections: Box<[Collection]>,
    sort_mode: SortMode,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
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
    #[must_use]
    pub const fn next(&self) -> Self {
        match self {
            Self::Name => Self::Name,
        }
    }

    #[must_use]
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
            tree_state: Mutex::new(CheckTreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut collections = state.library.collections.clone();
        self.props.sort_mode.sort_collections(&mut collections);
        let tree_state = if state.active_view == ActiveView::Collections {
            self.tree_state
        } else {
            Mutex::new(CheckTreeState::default())
        };

        Self {
            props: Props {
                collections,
                ..self.props
            },
            tree_state,
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
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
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

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 2));

        match kind {
            MouseEventKind::Down(MouseButton::Left) if area.contains(mouse_position) => {
                let Some(clicked) = self
                    .tree_state
                    .lock()
                    .unwrap()
                    .rendered_at(mouse_position)
                    .map(<[String]>::to_vec)
                else {
                    return;
                };

                if self.tree_state.lock().unwrap().select(clicked) {
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
                } else {
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            MouseEventKind::ScrollDown if area.contains(mouse_position) => {
                self.tree_state.lock().unwrap().scroll_down(1);
            }
            MouseEventKind::ScrollUp if area.contains(mouse_position) => {
                self.tree_state.lock().unwrap().scroll_up(1);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for LibraryCollectionsView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // render primary border
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Collections".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | s/S: change sort")
            .border_style(border_style);
        let content_area = border.inner(props.area);
        frame.render_widget(border, props.area);

        // draw additional border around content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP)
            .border_style(border_style);
        frame.render_widget(&border, content_area);
        let content_area = border.inner(content_area);

        // return the content area
        RenderProps {
            area: content_area,
            is_focused: props.is_focused,
        }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        // create a tree to hold the collections
        let items = self
            .props
            .collections
            .iter()
            .map(|collection| create_collection_tree_leaf(collection))
            .collect::<Vec<_>>();

        // render the collections
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

#[cfg(test)]
mod sort_mode_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::time::Duration;

    #[rstest]
    #[case(SortMode::Name, SortMode::Name)]
    fn test_sort_mode_next_prev(#[case] mode: SortMode, #[case] expected: SortMode) {
        assert_eq!(mode.next(), expected);
        assert_eq!(mode.next().prev(), mode);
    }

    #[rstest]
    #[case(SortMode::Name, "Name")]
    fn test_sort_mode_display(#[case] mode: SortMode, #[case] expected: &str) {
        assert_eq!(mode.to_string(), expected);
    }

    #[rstest]
    fn test_sort_collectionss() {
        let mut songs = vec![
            Collection {
                id: Collection::generate_id(),
                name: "C".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
            Collection {
                id: Collection::generate_id(),
                name: "A".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
            Collection {
                id: Collection::generate_id(),
                name: "B".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
        ];

        SortMode::Name.sort_collections(&mut songs);
        assert_eq!(songs[0].name, "A".into());
        assert_eq!(songs[1].name, "B".into());
        assert_eq!(songs[2].name, "C".into());
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
    use anyhow::Result;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = CollectionView::new(&state, tx);

        assert_eq!(view.name(), "Collection View");
        assert_eq!(
            view.props,
            Some(state.additional_view_data.collection.unwrap())
        );
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = CollectionView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(
            view.props,
            Some(new_state.additional_view_data.collection.unwrap())
        );
    }
    #[test]
    fn test_render_no_song() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = CollectionView::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(22, 3);
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
            "┌Collection View─────┐",
            "│No active collection│",
            "└────────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_render() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = CollectionView::new(&state_with_everything(), tx);

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
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on entire collection────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_render_with_checked() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = CollectionView::new(&state_with_everything(), tx);
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
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on entire collection────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // select the album
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on checked items────────────────────│",
            "│☑ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = CollectionView::new(&state_with_everything(), tx);

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
        let mut view = CollectionView::new(&state_with_everything(), tx);

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
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "collection",
                item_id()
            )
                .into()])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "collection",
                item_id()
            )
                .into()])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));

        // there are checked items
        // first we need to select an item (the album)
        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Song(item_id()))
        );

        // check the artist
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "song",
                item_id()
            )
                .into()])))
        );

        // add to collection
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "song",
                item_id()
            )
                .into()])))
        );
    }

    #[test]
    fn test_mouse_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = CollectionView::new(&state_with_everything(), tx);

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
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on entire collection────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
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
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on checked items────────────────────│",
            "│☑ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click down the song (opening it)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 6,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Song(item_id()))
        );
        let expected = Buffer::with_lines([
            "┌Collection View sorted by: Artist─────────────────────────┐",
            "│                       Collection 0                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                                                          │",
            "│q: add to queue | p: add to playlist──────────────────────│",
            "│Performing operations on entire collection────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);

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
    use anyhow::Result;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = LibraryCollectionsView::new(&state, tx);

        assert_eq!(view.name(), "Library Collections View");
        assert_eq!(view.props.collections, state.library.collections);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = LibraryCollectionsView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(view.props.collections, new_state.library.collections);
    }

    #[test]
    fn test_render() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = LibraryCollectionsView::new(&state_with_everything(), tx);

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
            "┌Library Collections sorted by: Name───────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│▪ Collection 0                                            │",
            "│                                                          │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | s/S: change sort──────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryCollectionsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, SortMode::Name);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SortMode::Name);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SortMode::Name);
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryCollectionsView::new(&state_with_everything(), tx);

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
        let mut view = LibraryCollectionsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        terminal.draw(|frame| view.render(frame, props)).unwrap();

        // first we need to navigate to the collection
        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Collection(item_id()))
        );
    }

    #[test]
    fn test_mouse_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryCollectionsView::new(&state_with_everything(), tx);

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
            "┌Library Collections sorted by: Name───────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│▪ Collection 0                                            │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | s/S: change sort──────────┘",
        ]);
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

        // click down the collection (opening it)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 3,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Collection(item_id()))
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
    }
}
