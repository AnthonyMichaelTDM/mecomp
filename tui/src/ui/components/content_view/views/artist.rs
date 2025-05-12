//! Views for both a single artist, and the library of artists.

use std::{ops::Not as _, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use mecomp_storage::db::schemas::artist::ArtistBrief;
use ratatui::{
    layout::{Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction, ViewAction},
    ui::{
        AppState,
        colors::{TEXT_HIGHLIGHT, border_color},
        components::{Component, ComponentRender, RenderProps, content_view::ActiveView},
        widgets::{
            popups::PopupType,
            tree::{CheckTree, state::CheckTreeState},
        },
    },
};

use super::{
    ArtistViewProps, checktree_utils::create_artist_tree_leaf, generic::ItemView,
    sort_mode::NameSort, traits::SortMode,
};

#[allow(clippy::module_name_repetitions)]
pub type ArtistView = ItemView<ArtistViewProps>;

pub struct LibraryArtistsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

struct Props {
    artists: Box<[ArtistBrief]>,
    sort_mode: NameSort<ArtistBrief>,
}
impl Component for LibraryArtistsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = NameSort::default();
        let mut artists = state.library.artists.clone();
        sort_mode.sort_items(&mut artists);
        Self {
            action_tx,
            props: Props { artists, sort_mode },
            tree_state: Mutex::new(CheckTreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut artists = state.library.artists.clone();
        self.props.sort_mode.sort_items(&mut artists);
        let tree_state = (state.active_view == ActiveView::Artists)
            .then_some(self.tree_state)
            .unwrap_or_default();

        Self {
            props: Props {
                artists,
                ..self.props
            },
            tree_state,
            ..self
        }
    }

    fn name(&self) -> &'static str {
        "Library Artists View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.artists.len().saturating_sub(1), |c| {
                        c.saturating_sub(10)
                    })
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
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::ActiveView(ViewAction::Set(thing.into())))
                            .unwrap();
                    }
                }
            }
            // when there are checked items, "q" will send the checked items to the queue
            KeyCode::Char('q') => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
                        .unwrap();
                }
            }
            // when there are checked items, "r" will start a radio with the checked items
            KeyCode::Char('r') => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::ActiveView(ViewAction::Set(ActiveView::Radio(
                            things,
                        ))))
                        .unwrap();
                }
            }
            // when there are checked items, "p" will send the checked items to the playlist
            KeyCode::Char('p') => {
                let things = self.tree_state.lock().unwrap().get_checked_things();
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
                self.props.sort_mode.sort_items(&mut self.props.artists);
                self.tree_state.lock().unwrap().scroll_selected_into_view();
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_items(&mut self.props.artists);
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
            .handle_mouse_event(mouse, area);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

impl ComponentRender<RenderProps> for LibraryArtistsView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        // draw primary border
        let border = Block::bordered()
            .title_top(Line::from(vec![
                Span::styled("Library Artists".to_string(), Style::default().bold()),
                Span::raw(" sorted by: "),
                Span::styled(self.props.sort_mode.to_string(), Style::default().italic()),
            ]))
            .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
            .border_style(border_style);
        let content_area = border.inner(props.area);
        frame.render_widget(border, props.area);

        // draw an additional border around the content area to display additional instructions
        let border = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .title_top(
                self.tree_state
                    .lock()
                    .unwrap()
                    .get_checked_things()
                    .is_empty()
                    .not()
                    .then_some("q: add to queue | r: start radio | p: add to playlist ")
                    .unwrap_or_default(),
            )
            .title_bottom("s/S: change sort")
            .border_style(border_style);
        let area = border.inner(content_area);
        frame.render_widget(border, content_area);

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        // create a tree for the artists
        let items = self
            .props
            .artists
            .iter()
            .map(create_artist_tree_leaf)
            .collect::<Vec<_>>();

        // render the artists
        frame.render_stateful_widget(
            CheckTree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

#[cfg(test)]
mod sort_mode_tests {
    use super::*;
    use mecomp_storage::db::schemas::artist::Artist;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(NameSort::default(), NameSort::default())]
    fn test_sort_mode_next_prev(
        #[case] mode: NameSort<ArtistBrief>,
        #[case] expected: NameSort<ArtistBrief>,
    ) {
        assert_eq!(mode.next(), expected);
        assert_eq!(mode.next().prev(), mode);
    }

    #[rstest]
    #[case(NameSort::default(), "Name")]
    fn test_sort_mode_display(#[case] mode: NameSort<ArtistBrief>, #[case] expected: &str) {
        assert_eq!(mode.to_string(), expected);
    }

    #[rstest]
    fn test_sort_items() {
        let mut artists = vec![
            ArtistBrief {
                id: Artist::generate_id(),
                name: "C".into(),
            },
            ArtistBrief {
                id: Artist::generate_id(),
                name: "B".into(),
            },
            ArtistBrief {
                id: Artist::generate_id(),
                name: "A".into(),
            },
        ];

        NameSort::default().sort_items(&mut artists);
        assert_eq!(artists[0].name, "A");
        assert_eq!(artists[1].name, "B");
        assert_eq!(artists[2].name, "C");
    }
}

#[cfg(test)]
mod item_view_tests {
    use super::*;
    use crate::test_utils::{
        assert_buffer_eq, item_id, setup_test_terminal, state_with_everything,
    };
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = ArtistView::new(&state, tx);

        assert_eq!(view.name(), "Artist View");
        assert_eq!(view.props, Some(state.additional_view_data.artist.unwrap()));
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = ArtistView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(
            view.props,
            Some(new_state.additional_view_data.artist.unwrap())
        );
    }

    #[test]
    fn test_render_no_artist() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = ArtistView::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(18, 3);
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
            "┌Artist View─────┐",
            "│No active artist│",
            "└────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = ArtistView::new(&state_with_everything(), tx);

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
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire artist────────────────────│",
            "│▶ Albums (1):                                             │",
            "│▶ Songs (1):                                              │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_checked() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = ArtistView::new(&state_with_everything(), tx);
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
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire artist────────────────────│",
            "│▶ Albums (1):                                             │",
            "│▶ Songs (1):                                              │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // select the song
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│▼ Songs (1):                                              │",
            "│  ☑ Test Song Test Artist                                 │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = ArtistView::new(&state_with_everything(), tx);

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
        let mut view = ArtistView::new(&state_with_everything(), tx);

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
                ("artist", item_id()).into()
            ])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("artist", item_id()).into()
            ],)))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("artist", item_id()).into()
            ])))
        );

        // there are checked items
        // first we need to select an item
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();
        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
        );

        // check the item
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("song", item_id()).into()
            ])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("song", item_id()).into()
            ],)))
        );

        // add to playlist
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
        let mut view = ArtistView::new(&state_with_everything(), tx);

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
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire artist────────────────────│",
            "│▶ Albums (1):                                             │",
            "│▶ Songs (1):                                              │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click on the dropdown
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
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on entire artist────────────────────│",
            "│▼ Albums (1):                                             │",
            "│  ☐ Test Album Test Artist                                │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
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

        // click down the checkbox item (which is already selected thanks to the scroll)
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 7,
                modifiers: KeyModifiers::empty(),
            },
            area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Album(item_id())))
        );
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Artist View───────────────────────────────────────────────┐",
            "│                        Test Artist                       │",
            "│        Albums: 1  Songs: 1  Duration: 00:03:00.00        │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│▼ Albums (1):                                             │",
            "│  ☑ Test Album Test Artist                                │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
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
    use crate::test_utils::{
        assert_buffer_eq, item_id, setup_test_terminal, state_with_everything,
    };
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = LibraryArtistsView::new(&state, tx);

        assert_eq!(view.name(), "Library Artists View");
        assert_eq!(view.props.artists, state.library.artists);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = LibraryArtistsView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(view.props.artists, new_state.library.artists);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = LibraryArtistsView::new(&state_with_everything(), tx);

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
            "┌Library Artists sorted by: Name───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Artist                                             │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_checked() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);
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
            "┌Library Artists sorted by: Name───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Artist                                             │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // check the first artist
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Artists sorted by: Name───────────────────────────┐",
            "│q: add to queue | r: start radio | p: add to playlist ────│",
            "│☑ Test Artist                                             │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, NameSort::default());
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, NameSort::default());
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);

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
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        terminal.draw(|frame| view.render(frame, props)).unwrap();

        // first we need to navigate to the artist
        view.handle_key_event(KeyEvent::from(KeyCode::Down));

        // now, we test the actions that require checked items when:
        // there are no checked items (order is different so that if an action is performed, the assertion later will fail)
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        // open
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveView(ViewAction::Set(ActiveView::Artist(item_id())))
        );

        // there are checked items
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("artist", item_id()).into()
            ])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("artist", item_id()).into()
            ],)))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("artist", item_id()).into()
            ])))
        );
    }

    #[test]
    fn test_mouse() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
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
            "┌Library Artists sorted by: Name───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Artist                                             │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // click on the album
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 2,
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
            "┌Library Artists sorted by: Name───────────────────────────┐",
            "│q: add to queue | r: start radio | p: add to playlist ────│",
            "│☑ Test Artist                                             │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // scroll down
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 2,
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
                row: 2,
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
            Action::ActiveView(ViewAction::Set(ActiveView::Artist(item_id())))
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
