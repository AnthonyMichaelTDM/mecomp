//! Views for both a single collection, and the library of collections.

// TODO: button to freeze the collection into a new playlist

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::collection::CollectionBrief;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, ViewAction},
    ui::{
        AppState,
        colors::{TEXT_HIGHLIGHT, TEXT_NORMAL, border_color},
        components::{Component, ComponentRender, RenderProps, content_view::ActiveView},
        widgets::tree::{CheckTree, state::CheckTreeState},
    },
};

use super::{
    CollectionViewProps,
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        create_collection_tree_leaf, create_song_tree_leaf,
    },
    sort_mode::{NameSort, SongSort},
    traits::SortMode,
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
    sort_mode: SongSort,
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
            sort_mode: SongSort::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.collection {
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
        "Collection View"
    }

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
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::ActiveView(ViewAction::Set(thing.into())))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, add them to the queue, otherwise send the whole collection to the queue
            KeyCode::Char('q') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_add_to_queue_action(
                    checked_things,
                    self.props.as_ref().map(|p| &p.id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to the playlist, otherwise send the whole collection to the playlist
            KeyCode::Char('p') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
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
        panic!("Failed to split collection view area")
    };

    [info_area, content_area]
}

impl ComponentRender<RenderProps> for CollectionView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

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
                        if self
                            .tree_state
                            .lock()
                            .unwrap()
                            .get_checked_things()
                            .is_empty()
                        {
                            "entire collection"
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
                .map(create_song_tree_leaf)
                .collect::<Vec<_>>();

            // render the collections songs
            frame.render_stateful_widget(
                CheckTree::new(&items)
                    .unwrap()
                    .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
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
                    .style(Style::default().fg((*TEXT_NORMAL).into()))
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
    collections: Box<[CollectionBrief]>,
    sort_mode: NameSort<CollectionBrief>,
}

impl Component for LibraryCollectionsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = NameSort::default();
        let mut collections = state.library.collections.clone();
        sort_mode.sort_items(&mut collections);
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
        self.props.sort_mode.sort_items(&mut collections);
        let tree_state = if state.active_view == ActiveView::Collections {
            self.tree_state
        } else {
            Mutex::default()
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

    fn name(&self) -> &'static str {
        "Library Collections View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    let first = self.props.collections.len().saturating_sub(1);
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
                self.props.sort_mode.sort_items(&mut self.props.collections);
                self.tree_state.lock().unwrap().scroll_selected_into_view();
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_items(&mut self.props.collections);
                self.tree_state.lock().unwrap().scroll_selected_into_view();
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 2));

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

impl ComponentRender<RenderProps> for LibraryCollectionsView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

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
            .map(create_collection_tree_leaf)
            .collect::<Vec<_>>();

        // render the collections
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
    use mecomp_storage::db::schemas::collection::Collection;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(NameSort::default(), NameSort::default())]
    fn test_sort_mode_next_prev(
        #[case] mode: NameSort<CollectionBrief>,
        #[case] expected: NameSort<CollectionBrief>,
    ) {
        assert_eq!(mode.next(), expected);
        assert_eq!(mode.next().prev(), mode);
    }

    #[rstest]
    #[case(NameSort::default(), "Name")]
    fn test_sort_mode_display(#[case] mode: NameSort<CollectionBrief>, #[case] expected: &str) {
        assert_eq!(mode.to_string(), expected);
    }

    #[rstest]
    fn test_sort_collectionss() {
        let mut collections = vec![
            CollectionBrief {
                id: Collection::generate_id(),
                name: "C".into(),
            },
            CollectionBrief {
                id: Collection::generate_id(),
                name: "A".into(),
            },
            CollectionBrief {
                id: Collection::generate_id(),
                name: "B".into(),
            },
        ];

        NameSort::default().sort_items(&mut collections);
        assert_eq!(collections[0].name, "A");
        assert_eq!(collections[1].name, "B");
        assert_eq!(collections[2].name, "C");
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
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
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
    fn test_render_no_collection() {
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
    }

    #[test]
    fn test_render() {
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
    }

    #[test]
    fn test_render_with_checked() {
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
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("collection", item_id()).into()
            ])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("collection", item_id()).into()
            ])))
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

        // add to collection
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("song", item_id()).into()
            ])))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
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

        // click down the song again
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
        // ctrl click on it (opening it)
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
    }
}

#[cfg(test)]
mod library_view_tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
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
    fn test_render() {
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
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryCollectionsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, NameSort::default());
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
            Action::ActiveView(ViewAction::Set(ActiveView::Collection(item_id())))
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

        // click on the collection when it's not selected
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
            Action::ActiveView(ViewAction::Set(ActiveView::Collection(item_id())))
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

        // click down the collection (opening it)
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
            Action::ActiveView(ViewAction::Set(ActiveView::Collection(item_id())))
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
                row: 2,
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
            Action::ActiveView(ViewAction::Set(ActiveView::Collection(item_id())))
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
