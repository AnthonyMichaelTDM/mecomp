//! implementation the search view

use std::sync::Mutex;

use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::SearchResult;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction, ViewAction},
    ui::{
        AppState,
        colors::{TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL, border_color},
        components::{Component, ComponentRender, RenderProps, content_view::ActiveView},
        widgets::{
            input_box::{self, InputBox},
            popups::PopupType,
            tree::{CheckTree, state::CheckTreeState},
        },
    },
};

use super::checktree_utils::{
    create_album_tree_item, create_artist_tree_item, create_song_tree_item,
};

#[allow(clippy::module_name_repetitions)]
pub struct SearchView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// Search Bar
    search_bar: InputBox,
    /// Is the search bar focused
    search_bar_focused: bool,
}

pub struct Props {
    pub(crate) search_results: SearchResult,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        Self {
            search_results: value.search.clone(),
        }
    }
}

impl Component for SearchView {
    fn new(
        state: &AppState,
        action_tx: tokio::sync::mpsc::UnboundedSender<crate::state::action::Action>,
    ) -> Self
    where
        Self: Sized,
    {
        let props = Props::from(state);
        Self {
            search_bar: InputBox::new(state, action_tx.clone()),
            search_bar_focused: true,
            tree_state: Mutex::new(CheckTreeState::default()),
            action_tx,
            props,
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        Self {
            search_bar: self.search_bar.move_with_state(state),
            props: Props::from(state),
            tree_state: Mutex::new(CheckTreeState::default()),
            ..self
        }
    }

    fn name(&self) -> &'static str {
        "Search"
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    let first = self.props.search_results.len().saturating_sub(1);
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
            KeyCode::Left if !self.search_bar_focused => {
                self.tree_state.lock().unwrap().key_left();
            }
            KeyCode::Right if !self.search_bar_focused => {
                self.tree_state.lock().unwrap().key_right();
            }
            KeyCode::Char(' ') if !self.search_bar_focused => {
                self.tree_state.lock().unwrap().key_space();
            }
            // when searchbar focused, enter key will search
            KeyCode::Enter if self.search_bar_focused => {
                self.search_bar_focused = false;
                self.tree_state.lock().unwrap().reset();
                if !self.search_bar.is_empty() {
                    self.action_tx
                        .send(Action::Search(self.search_bar.text().to_string()))
                        .unwrap();
                    self.search_bar.reset();
                }
            }
            KeyCode::Char('/') if !self.search_bar_focused => {
                self.search_bar_focused = true;
            }
            // when searchbar unfocused, enter key will open the selected node
            KeyCode::Enter if !self.search_bar_focused => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::ActiveView(ViewAction::Set(thing.into())))
                            .unwrap();
                    }
                }
            }
            // when search bar unfocused, and there are checked items, "q" will send the checked items to the queue
            KeyCode::Char('q') if !self.search_bar_focused => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
                        .unwrap();
                }
            }
            // when search bar unfocused, and there are checked items, "r" will start a radio with the checked items
            KeyCode::Char('r') if !self.search_bar_focused => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::ActiveView(ViewAction::Set(ActiveView::Radio(
                            things,
                        ))))
                        .unwrap();
                }
            }
            // when search bar unfocused, and there are checked items, "p" will send the checked items to the playlist
            KeyCode::Char('p') if !self.search_bar_focused => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(
                            things,
                        ))))
                        .unwrap();
                }
            }

            // defer to the search bar, if it is focused
            _ if self.search_bar_focused => {
                self.search_bar.handle_key_event(key);
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        // split the area into search bar and content area
        let [search_bar_area, content_area] = split_area(area);

        match (self.search_bar_focused, kind) {
            // defer to the search bar if mouse event belongs to it
            (true, _) if search_bar_area.contains(mouse_position) => {
                self.search_bar.handle_mouse_event(mouse, search_bar_area);
            }
            // if the search bar is focused and mouse is clicked outside of it, unfocus the search bar
            (true, MouseEventKind::Down(MouseButton::Left))
                if content_area.contains(mouse_position) =>
            {
                self.search_bar_focused = false;
            }
            // if the search bar is not focused and mouse is clicked inside it, focus the search bar
            (false, MouseEventKind::Down(MouseButton::Left))
                if search_bar_area.contains(mouse_position) =>
            {
                self.search_bar_focused = true;
            }
            // defer to the tree state for mouse events in the content area when the search bar is not focused
            (false, _) if content_area.contains(mouse_position) => {
                // adjust the content area to exclude the border
                let content_area = Rect {
                    x: content_area.x.saturating_add(1),
                    y: content_area.y.saturating_add(1),
                    width: content_area.width.saturating_sub(1),
                    height: content_area.height.saturating_sub(2),
                };

                let result =
                    self.tree_state
                        .lock()
                        .unwrap()
                        .handle_mouse_event(mouse, content_area, false);
                if let Some(action) = result {
                    self.action_tx.send(action).unwrap();
                }
            }
            _ => {}
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [search_bar_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)].as_ref())
        .areas(area);
    [search_bar_area, content_area]
}

impl ComponentRender<RenderProps> for SearchView {
    fn render_border(&self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style =
            Style::default().fg(border_color(props.is_focused && !self.search_bar_focused).into());

        // split view
        let [search_bar_area, content_area] = split_area(props.area);

        // render the search bar
        self.search_bar.render(
            frame,
            input_box::RenderProps {
                area: search_bar_area,
                text_color: if self.search_bar_focused {
                    (*TEXT_HIGHLIGHT_ALT).into()
                } else {
                    (*TEXT_NORMAL).into()
                },
                border: Block::bordered().title("Search").border_style(
                    Style::default()
                        .fg(border_color(self.search_bar_focused && props.is_focused).into()),
                ),
                show_cursor: self.search_bar_focused,
            },
        );

        // put a border around the content area
        let area = if self.search_bar_focused {
            let border = Block::bordered()
                .title_top("Results")
                .title_bottom(" \u{23CE} : Search")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            border.inner(content_area)
        } else {
            let border = Block::bordered()
                .title_top("Results")
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            let content_area = border.inner(content_area);

            let border = Block::default()
                .borders(Borders::BOTTOM)
                .title_bottom("/: Search | \u{2423} : Check")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            border.inner(content_area)
        };

        // if there are checked items, put an additional border around the content area to display additional instructions
        let area = if self
            .tree_state
            .lock()
            .unwrap()
            .get_checked_things()
            .is_empty()
        {
            area
        } else {
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top("q: add to queue | r: start radio | p: add to playlist")
                .border_style(border_style);
            frame.render_widget(&border, area);
            border.inner(area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
        // if there are no search results, render a message
        if self.props.search_results.is_empty() {
            frame.render_widget(
                Line::from("No results found")
                    .style(Style::default().fg((*TEXT_NORMAL).into()))
                    .alignment(Alignment::Center),
                props.area,
            );
            return;
        }

        // create tree to hold results
        let song_tree = create_song_tree_item(&self.props.search_results.songs).unwrap();
        let album_tree = create_album_tree_item(&self.props.search_results.albums).unwrap();
        let artist_tree = create_artist_tree_item(&self.props.search_results.artists).unwrap();
        let items = &[song_tree, album_tree, artist_tree];

        // render the search results
        frame.render_stateful_widget(
            CheckTree::new(items)
                .unwrap()
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use mecomp_prost::RecordId;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_render_search_focused() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(24, 8);
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
            "┌Search────────────────┐",
            "│                      │",
            "└──────────────────────┘",
            "┌Results───────────────┐",
            "│▶ Songs (1):          │",
            "│▶ Albums (1):         │",
            "│▶ Artists (1):        │",
            "└ ⏎ : Search───────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_empty() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            search: SearchResult::default(),
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(24, 8);
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
            "┌Search────────────────┐",
            "│                      │",
            "└──────────────────────┘",
            "┌Results───────────────┐",
            "│   No results found   │",
            "│                      │",
            "│                      │",
            "└ ⏎ : Search───────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_search_unfocused() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(32, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };

        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│                              │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        view.handle_key_event(KeyEvent::from(KeyCode::PageUp));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
        view.handle_key_event(KeyEvent::from(KeyCode::Left));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_keys() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(32, 10);
        let props = RenderProps {
            area,
            is_focused: true,
        };

        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│qrp                           │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│                              │",
            "│                              │",
            "└ ⏎ : Search───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(action, Action::Search("qrp".to_string()));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│                              │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│                              │",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Char('/')));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│                              │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▼ Songs (1):                  │",
            "│  ☐ Test Song Test Artist     │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│                              │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│q: add to queue | r: start rad│",
            "│▼ Songs (1):                 ▲│",
            "│  ☑ Test Song Test Artist    █│",
            "│▶ Albums (1):                ▼│",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![RecordId::new(
                "song",
                item_id()
            )])))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![RecordId::new(
                "song",
                item_id()
            )],)))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![RecordId::new(
                "song",
                item_id()
            )])))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_mouse() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(32, 10);
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
            "┌Search────────────────────────┐",
            "│                              │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│                              │",
            "│                              │",
            "└ ⏎ : Search───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // put some text in the search bar
        view.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('b')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('c')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│abc                           │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│                              │",
            "│                              │",
            "└ ⏎ : Search───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click in the search bar and ensure the cursor is moved to the right place
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('c')));
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│acbc                          │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▶ Albums (1):                 │",
            "│▶ Artists (1):                │",
            "│                              │",
            "│                              │",
            "└ ⏎ : Search───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        // click out of the search bar
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 5,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // scroll down
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // click on the selected dropdown
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 5,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│acbc                          │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▶ Songs (1):                  │",
            "│▼ Albums (1):                 │",
            "│  ☐ Test Album Test Artist    │",
            "│▶ Artists (1):                │",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
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
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // click on the selected dropdown
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│acbc                          │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│▼ Songs (1):                 ▲│",
            "│  ☐ Test Song Test Artist    █│",
            "│▼ Albums (1):                █│",
            "│  ☐ Test Album Test Artist   ▼│",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
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
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );

        // ctrl-click
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 5,
                modifiers: KeyModifiers::CONTROL,
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id().into())))
        );
        let expected = Buffer::with_lines([
            "┌Search────────────────────────┐",
            "│acbc                          │",
            "└──────────────────────────────┘",
            "┌Results───────────────────────┐",
            "│q: add to queue | r: start rad│",
            "│▼ Songs (1):                 ▲│",
            "│  ☑ Test Song Test Artist    █│",
            "│▼ Albums (1):                ▼│",
            "│/: Search | ␣ : Check─────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate─┘",
        ]);
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        assert_buffer_eq(&buffer, &expected);
    }
}
