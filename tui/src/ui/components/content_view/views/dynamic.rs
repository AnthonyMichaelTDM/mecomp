use std::{str::FromStr, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::{DynamicPlaylist, SongBrief};
use mecomp_storage::db::schemas::dynamic::query::Query;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, ViewAction},
    ui::{
        AppState,
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
            border_color,
        },
        components::{
            Component, ComponentRender, RenderProps,
            content_view::{ActiveView, views::generic::SortableItemView},
        },
        widgets::{
            input_box::{self, InputBox},
            tree::{CheckTree, state::CheckTreeState},
        },
    },
};

use super::{
    DynamicPlaylistViewProps,
    checktree_utils::create_dynamic_playlist_tree_leaf,
    sort_mode::{NameSort, SongSort},
    traits::SortMode,
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

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

#[allow(clippy::module_name_repetitions)]
pub type DynamicView = SortableItemView<DynamicPlaylistViewProps, SongSort, SongBrief>;

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
    pub dynamics: Vec<DynamicPlaylist>,
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
        // one for navigating the tree when the input boxes are not visible
        // one for interacting with the input boxes when they are visible
        if self.focus == Focus::Tree {
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
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
                }
                KeyCode::Char('S') => {
                    self.props.sort_mode = self.props.sort_mode.prev();
                    self.props.sort_mode.sort_items(&mut self.props.dynamics);
                    self.tree_state.lock().unwrap().scroll_selected_into_view();
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
                            .send(Action::Library(LibraryAction::RemoveDynamicPlaylist(
                                thing.ulid(),
                            )))
                            .unwrap();
                    }
                }
                _ => {}
            }
        } else {
            let query = Query::from_str(self.query_builder.text()).ok();
            let name = self.name_input_box.text();

            match (key.code, query, self.focus) {
                // if the user presses Enter with an empty name, we cancel the operation
                (KeyCode::Enter, _, Focus::NameInput) if name.is_empty() => {
                    self.focus = Focus::Tree;
                }
                // if the user pressed Enter with a valid (non-empty) name, we prompt the user for the query
                (KeyCode::Enter, _, Focus::NameInput) if !name.is_empty() => {
                    self.focus = Focus::QueryInput;
                }
                // if the user presses Enter with a valid query, we create a new playlist
                (KeyCode::Enter, Some(query), Focus::QueryInput) => {
                    self.action_tx
                        .send(Action::Library(LibraryAction::CreateDynamicPlaylist(
                            name.to_string(),
                            query,
                        )))
                        .unwrap();
                    self.name_input_box.reset();
                    self.query_builder.reset();
                    self.focus = Focus::Tree;
                }
                // otherwise defer to the focused input box
                (_, _, Focus::NameInput) => self.name_input_box.handle_key_event(key),
                (_, _, Focus::QueryInput) => self.query_builder.handle_key_event(key),
                (_, _, Focus::Tree) => unreachable!(),
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
                .handle_mouse_event(mouse, area, true);
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
    fn render_border(&self, frame: &mut Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        // render primary border
        let border_title_bottom = if self.focus == Focus::Tree {
            " \u{23CE} : Open | ←/↑/↓/→: Navigate | s/S: change sort"
        } else {
            ""
        };
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled(
                    "Library Dynamic Playlists".to_string(),
                    Style::default().bold(),
                ),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(border_title_bottom)
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
                Focus::NameInput => ((*TEXT_HIGHLIGHT_ALT).into(), (*TEXT_HIGHLIGHT).into()),
                Focus::QueryInput => ((*TEXT_HIGHLIGHT).into(), (*TEXT_HIGHLIGHT_ALT).into()),
                Focus::Tree => ((*TEXT_NORMAL).into(), (*TEXT_NORMAL).into()),
            };

            let (name_border_color, query_border_color) = match self.focus {
                Focus::NameInput => ((*BORDER_FOCUSED).into(), (*BORDER_UNFOCUSED).into()),
                Focus::QueryInput => ((*BORDER_UNFOCUSED).into(), (*BORDER_FOCUSED).into()),
                Focus::Tree => ((*BORDER_UNFOCUSED).into(), (*BORDER_UNFOCUSED).into()),
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
            let query_builder_props = if Query::from_str(self.query_builder.text()).is_ok() {
                input_box::RenderProps {
                    area: query_builder_area,
                    text_color: query_text_color,
                    border: Block::bordered()
                        .title("Enter Query:")
                        .border_style(Style::default().fg(query_border_color)),
                    show_cursor: query_show_cursor,
                }
            } else {
                input_box::RenderProps {
                    area: query_builder_area,
                    text_color: (*TEXT_HIGHLIGHT).into(),
                    border: Block::bordered()
                        .title("Invalid Query:")
                        .border_style(Style::default().fg(query_border_color)),
                    show_cursor: query_show_cursor,
                }
            };
            self.query_builder.inner.render(frame, query_builder_props);

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

    fn render_content(&self, frame: &mut Frame<'_>, props: RenderProps) {
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
mod item_view_tests {
    use super::*;
    use crate::{
        state::action::{AudioAction, PopupAction, QueueAction},
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::{components::content_view::ActiveView, widgets::popups::PopupType},
    };
    use crossterm::event::KeyModifiers;
    use mecomp_prost::RecordId;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn test_new() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = DynamicView::new(&state, tx).item_view;

        assert_eq!(view.name(), "Dynamic Playlist View");
        assert_eq!(
            view.props,
            Some(state.additional_view_data.dynamic_playlist.unwrap())
        );
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = unbounded_channel();
        let state = AppState::default();
        let view = DynamicView::new(&state, tx);

        let new_state = state_with_everything();
        let new_view = view.move_with_state(&new_state).item_view;

        assert_eq!(
            new_view.props,
            Some(new_state.additional_view_data.dynamic_playlist.unwrap())
        );
    }

    #[test]
    fn test_name() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = DynamicView::new(&state, tx);

        assert_eq!(view.name(), "Dynamic Playlist View");
    }

    #[test]
    fn smoke_navigation_and_sort() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let mut view = DynamicView::new(&state, tx);

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
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = DynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // when there are no checked items:
        // - Enter should do nothing
        // - "q" should add the entire playlist to the queue
        // - "r" should start radio from the entire playlist
        // - "p" should add the entire playlist to a playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let dynamic_playlists_id = state
            .additional_view_data
            .dynamic_playlist
            .as_ref()
            .unwrap()
            .id
            .clone();
        assert_eq!(
            rx.blocking_recv(),
            Some(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                dynamic_playlists_id.clone()
            ]))))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                dynamic_playlists_id.clone()
            ],)))
        );
        assert_eq!(
            rx.blocking_recv(),
            Some(Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                dynamic_playlists_id
            ]))))
        );

        // check the only item in the playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // when there are checked items:
        // - Enter should open the selected view
        // - "q" should add the checked items to the queue
        // - "r" should start radio from the checked items
        // - "p" should add the checked items to a playlist
        let song_id: RecordId = ("song", item_id()).into();
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![song_id.clone()])))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![song_id.clone()],)))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![song_id])))
        );
    }

    #[test]
    fn test_edit() {
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = DynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // "e" key should open the dynamic playlist editor popup
        view.handle_key_event(KeyEvent::from(KeyCode::Char('e')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::DynamicPlaylistEditor(
                state
                    .additional_view_data
                    .dynamic_playlist
                    .as_ref()
                    .unwrap()
                    .dynamic_playlist
                    .clone()
            )))
        );
    }

    #[test]
    fn test_mouse_events() {
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = DynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // click on the song (selecting it)
        assert_eq!(
            view.item_view
                .tree_state
                .lock()
                .unwrap()
                .get_selected_thing(),
            None
        );
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
            view.item_view
                .tree_state
                .lock()
                .unwrap()
                .get_selected_thing(),
            Some(("song", item_id()).into())
        );

        // ctrl-click on the selected song (opening it)
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

    #[test]
    fn test_render_no_dynamic_playlist() {
        let (tx, _) = unbounded_channel();
        let state = AppState::default();
        let view = DynamicView::new(&state, tx);

        let (mut terminal, area) = setup_test_terminal(28, 3);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌Dynamic Playlist View─────┐",
            "│No active dynamic playlist│",
            "└──────────────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = DynamicView::new(&state, tx);

        let (mut terminal, area) = setup_test_terminal(60, 10);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Dynamic Playlist View sorted by: Artist───────────────────┐",
            "│                       Test Dynamic                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                    title = \"Test Song\"                   │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire dynamic playlist──────────│",
            "│☐ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: sort | e: edit───────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_checked() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let mut view = DynamicView::new(&state, tx);
        let (mut terminal, area) = setup_test_terminal(60, 10);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Dynamic Playlist View sorted by: Artist───────────────────┐",
            "│                       Test Dynamic                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                    title = \"Test Song\"                   │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire dynamic playlist──────────│",
            "│☐ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: sort | e: edit───────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // select the song
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Dynamic Playlist View sorted by: Artist───────────────────┐",
            "│                       Test Dynamic                       │",
            "│              Songs: 1  Duration: 00:03:00.00             │",
            "│                    title = \"Test Song\"                   │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│☑ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: sort | e: edit───────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }
}

#[cfg(test)]
mod library_view_tests {
    use super::*;
    use crate::{
        state::action::{LibraryAction, ViewAction},
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
    };
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn test_new() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = LibraryDynamicView::new(&state, tx);

        assert_eq!(view.name(), "Library Dynamic Playlists View");
        assert_eq!(view.props.dynamics, state.library.dynamic_playlists);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = unbounded_channel();
        let state = AppState::default();
        let view = LibraryDynamicView::new(&state, tx);

        let new_state = state_with_everything();
        let new_view = view.move_with_state(&new_state);

        assert_eq!(new_view.props.dynamics, new_state.library.dynamic_playlists);
    }

    #[test]
    fn test_name() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = LibraryDynamicView::new(&state, tx);

        assert_eq!(view.name(), "Library Dynamic Playlists View");
    }

    #[test]
    fn smoke_navigation_and_sort() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let mut view = LibraryDynamicView::new(&state, tx);

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
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = LibraryDynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // when there are no selected items:
        // - Enter should do nothing
        // - "d" should do nothing
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));

        // select the dynamic playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        // when there are selected items:
        // - "d" should delete the selected dynamic playlist
        // - Enter should open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::RemoveDynamicPlaylist(item_id()))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::DynamicPlaylist(item_id())))
        );
    }

    #[test]
    fn test_actions_with_input_boxes() {
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = LibraryDynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // pressing "n" should reveal the name/query input boxes
        assert_eq!(view.focus, Focus::Tree);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(view.focus, Focus::NameInput);

        // when the name input box is focused:
        // - Enter with an empty name should cancel the operation
        // - Enter with a valid name should reveal the query input box
        // - other keys are deferred to the input box
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(view.focus, Focus::Tree);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('n'))); // reveal the name input box again
        assert_eq!(view.focus, Focus::NameInput);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('b')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('c')));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(view.name_input_box.text(), "abc");
        assert_eq!(view.focus, Focus::QueryInput);

        // when the query input box is focused:
        // - Enter with an invalid query should do nothing
        // - Enter with a valid query should create a new dynamic playlist
        // - other keys are deferred to the input box
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(view.focus, Focus::QueryInput);
        let query = "artist CONTAINS 'foo'";
        for c in query.chars() {
            view.handle_key_event(KeyEvent::from(KeyCode::Char(c)));
        }
        assert_eq!(view.query_builder.text(), query);
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::CreateDynamicPlaylist(
                "abc".to_string(),
                Query::from_str(query).unwrap()
            ))
        );
    }

    #[test]
    fn test_mouse_events() {
        let (tx, mut rx) = unbounded_channel();
        let state = state_with_everything();
        let mut view = LibraryDynamicView::new(&state, tx);

        // need to render at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|f| view.render(f, props)).unwrap();

        // without the input boxes visible:
        // - clicking on the tree should open the clicked item
        // - clicking on an empty area should do nothing
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::empty(),
        };
        view.handle_mouse_event(mouse_event, area); // open
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::DynamicPlaylist(item_id())))
        );
        // clicking on an empty area should clear the selection
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 3,
            modifiers: KeyModifiers::empty(),
        };
        view.handle_mouse_event(mouse_event, area);
        assert_eq!(view.tree_state.lock().unwrap().get_selected_thing(), None);
        view.handle_mouse_event(mouse_event, area);
        assert_eq!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        );
        // ctrl-clicking on the tree should just change the selection
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
                modifiers: KeyModifiers::CONTROL,
            },
            area,
        );
        assert_eq!(
            view.tree_state.lock().unwrap().get_selected_thing(),
            Some(("dynamic", item_id()).into())
        );
        assert_eq!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Char('n'))); // reveal the name input box

        // with the input boxes visible:
        // - clicking on the query input box should focus it
        // - clicking on the name input box should focus it
        // - clicking on the content area should defocus and hide the input boxes
        assert_eq!(view.focus, Focus::NameInput);
        view.handle_mouse_event(
            // click on the query input box
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 5,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(view.focus, Focus::QueryInput);
        view.handle_mouse_event(
            // click on the name input box
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(view.focus, Focus::NameInput);
        // click on the content area
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 8,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(view.focus, Focus::Tree);
    }

    #[test]
    fn test_render() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let view = LibraryDynamicView::new(&state, tx);

        let (mut terminal, area) = setup_test_terminal(60, 6);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Dynamic Playlists sorted by: Name─────────────────┐",
            "│n: new dynamic | d: delete dynamic────────────────────────│",
            "│▪ Test Dynamic                                            │",
            "│                                                          │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | s/S: change sort──────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_input_boxes_visible() {
        let (tx, _) = unbounded_channel();
        let state = state_with_everything();
        let mut view = LibraryDynamicView::new(&state, tx);

        // reveal the name input box
        view.handle_key_event(KeyEvent::from(KeyCode::Char('n')));

        let (mut terminal, area) = setup_test_terminal(60, 11);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Dynamic Playlists sorted by: Name─────────────────┐",
            "│┌Enter Name:─────────────────────────────────────────────┐│",
            "││                                                        ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│┌Invalid Query:──────────────────────────────────────────┐│",
            "││                                                        ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│ ⏎ : Set (cancel if empty)────────────────────────────────│",
            "│▪ Test Dynamic                                            │",
            "│                                                          │",
            "└──────────────────────────────────────────────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        let name = "Test";
        for c in name.chars() {
            view.handle_key_event(KeyEvent::from(KeyCode::Char(c)));
        }
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Dynamic Playlists sorted by: Name─────────────────┐",
            "│┌Enter Name:─────────────────────────────────────────────┐│",
            "││Test                                                    ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│┌Invalid Query:──────────────────────────────────────────┐│",
            "││                                                        ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│ ⏎ : Create (cancel if empty)─────────────────────────────│",
            "│▪ Test Dynamic                                            │",
            "│                                                          │",
            "└──────────────────────────────────────────────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        let query = "artist CONTAINS 'foo'";
        for c in query.chars() {
            view.handle_key_event(KeyEvent::from(KeyCode::Char(c)));
        }

        let buffer = terminal
            .draw(|f| view.render(f, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Dynamic Playlists sorted by: Name─────────────────┐",
            "│┌Enter Name:─────────────────────────────────────────────┐│",
            "││Test                                                    ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│┌Enter Query:────────────────────────────────────────────┐│",
            "││artist CONTAINS 'foo'                                   ││",
            "│└────────────────────────────────────────────────────────┘│",
            "│ ⏎ : Create (cancel if empty)─────────────────────────────│",
            "│▪ Test Dynamic                                            │",
            "│                                                          │",
            "└──────────────────────────────────────────────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }
}
