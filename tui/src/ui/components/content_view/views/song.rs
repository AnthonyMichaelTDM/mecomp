//! Views for both a single song, and the library of songs.

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use mecomp_storage::db::schemas::song::SongBrief;
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
    SongViewProps, checktree_utils::create_song_tree_leaf, generic::ItemView, sort_mode::SongSort,
    traits::SortMode,
};

#[allow(clippy::module_name_repetitions)]
pub type SongView = ItemView<SongViewProps>;

pub struct LibrarySongsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub(crate) props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

pub(crate) struct Props {
    pub(crate) songs: Box<[SongBrief]>,
    pub(crate) sort_mode: SongSort,
}

impl Component for LibrarySongsView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let sort_mode = SongSort::default();
        let mut songs = state.library.songs.clone();
        sort_mode.sort_items(&mut songs);
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
        self.props.sort_mode.sort_items(&mut songs);
        let tree_state = if state.active_view == ActiveView::Songs {
            self.tree_state
        } else {
            Mutex::default()
        };

        Self {
            props: Props {
                songs,
                ..self.props
            },
            tree_state,
            ..self
        }
    }

    fn name(&self) -> &'static str {
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
                self.props.sort_mode.sort_items(&mut self.props.songs);
                self.tree_state.lock().unwrap().scroll_selected_into_view();
            }
            KeyCode::Char('S') => {
                self.props.sort_mode = self.props.sort_mode.prev();
                self.props.sort_mode.sort_items(&mut self.props.songs);
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
            .handle_mouse_event(mouse, area, false);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

impl ComponentRender<RenderProps> for LibrarySongsView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

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
        let tree_checked_things_empty = self
            .tree_state
            .lock()
            .unwrap()
            .get_checked_things()
            .is_empty();
        let border_title_top = if tree_checked_things_empty {
            ""
        } else {
            "q: add to queue | r: start radio | p: add to playlist "
        };
        let border = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .title_top(border_title_top)
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
            .map(create_song_tree_leaf)
            .collect::<Vec<_>>();

        // render the tree
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
    use mecomp_storage::db::schemas::song::Song;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::time::Duration;

    #[rstest]
    #[case(SongSort::Title, SongSort::Artist)]
    #[case(SongSort::Artist, SongSort::Album)]
    #[case(SongSort::Album, SongSort::AlbumArtist)]
    #[case(SongSort::AlbumArtist, SongSort::Genre)]
    #[case(SongSort::Genre, SongSort::Title)]
    fn test_sort_mode_next_prev(#[case] mode: SongSort, #[case] expected: SongSort) {
        assert_eq!(mode.next(), expected);
        assert_eq!(mode.next().prev(), mode);
    }

    #[rstest]
    #[case(SongSort::Title, "Title")]
    #[case(SongSort::Artist, "Artist")]
    #[case(SongSort::Album, "Album")]
    #[case(SongSort::AlbumArtist, "Album Artist")]
    #[case(SongSort::Genre, "Genre")]
    fn test_sort_mode_display(#[case] mode: SongSort, #[case] expected: &str) {
        assert_eq!(mode.to_string(), expected);
    }

    #[rstest]
    fn test_sort_songs() {
        let mut songs = vec![
            SongBrief {
                id: Song::generate_id(),
                title: "C".into(),
                artist: "B".to_string().into(),
                album: "A".into(),
                album_artist: "C".to_string().into(),
                genre: "B".to_string().into(),
                runtime: Duration::from_secs(180),
                track: Some(1),
                disc: Some(1),
                release_year: Some(2021),
                extension: "mp3".into(),
                path: "test.mp3".into(),
            },
            SongBrief {
                id: Song::generate_id(),
                title: "B".into(),
                artist: "A".to_string().into(),
                album: "C".into(),
                album_artist: "B".to_string().into(),
                genre: "A".to_string().into(),
                runtime: Duration::from_secs(180),
                track: Some(1),
                disc: Some(1),
                release_year: Some(2021),
                extension: "mp3".into(),
                path: "test.mp3".into(),
            },
            SongBrief {
                id: Song::generate_id(),
                title: "A".into(),
                artist: "C".to_string().into(),
                album: "B".into(),
                album_artist: "A".to_string().into(),
                genre: "C".to_string().into(),
                runtime: Duration::from_secs(180),
                track: Some(1),
                disc: Some(1),
                release_year: Some(2021),
                extension: "mp3".into(),
                path: "test.mp3".into(),
            },
        ];

        SongSort::Title.sort_items(&mut songs);
        assert_eq!(songs[0].title, "A");
        assert_eq!(songs[1].title, "B");
        assert_eq!(songs[2].title, "C");

        SongSort::Artist.sort_items(&mut songs);
        assert_eq!(songs[0].artist, "A".to_string().into());
        assert_eq!(songs[1].artist, "B".to_string().into());
        assert_eq!(songs[2].artist, "C".to_string().into());

        SongSort::Album.sort_items(&mut songs);
        assert_eq!(songs[0].album, "A");
        assert_eq!(songs[1].album, "B");
        assert_eq!(songs[2].album, "C");

        SongSort::AlbumArtist.sort_items(&mut songs);
        assert_eq!(songs[0].album_artist, "A".to_string().into());
        assert_eq!(songs[1].album_artist, "B".to_string().into());
        assert_eq!(songs[2].album_artist, "C".to_string().into());

        SongSort::Genre.sort_items(&mut songs);
        assert_eq!(songs[0].genre, "A".to_string().into());
        assert_eq!(songs[1].genre, "B".to_string().into());
        assert_eq!(songs[2].genre, "C".to_string().into());
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
        let view = SongView::new(&state, tx);

        assert_eq!(view.name(), "Song View");
        assert_eq!(view.props, Some(state.additional_view_data.song.unwrap()));
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = SongView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(
            view.props,
            Some(new_state.additional_view_data.song.unwrap())
        );
    }

    #[test]
    fn test_render_no_song() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = SongView::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(16, 3);
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
            "┌Song View─────┐",
            "│No active song│",
            "└──────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_render_no_playlist_no_collection() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut state = state_with_everything();
        state.additional_view_data.song.as_mut().unwrap().playlists = [].into();
        state
            .additional_view_data
            .song
            .as_mut()
            .unwrap()
            .collections = [].into();
        let mut view = SongView::new(&state, tx);

        let (mut terminal, area) = setup_test_terminal(60, 12);
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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
            "│▶ Playlists (0):                                          │",
            "│▶ Collections (0):                                        │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
        assert!(view.tree_state.lock().unwrap().selected().is_empty());

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(view.tree_state.lock().unwrap().selected(), &["Artists"]);
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(
            view.tree_state.lock().unwrap().selected(),
            &[state.library.albums[0].id.to_string()]
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(view.tree_state.lock().unwrap().selected(), &["Playlists"]);
        view.handle_key_event(KeyEvent::from(KeyCode::Right));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
            "│▼ Playlists (0):                                          │",
            "│                                                          │",
            "│▶ Collections (0):                                        │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Left));
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
            "│▶ Playlists (0):                                          │",
            "│▶ Collections (0):                                        │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(view.tree_state.lock().unwrap().selected(), &["Collections"]);
        view.handle_key_event(KeyEvent::from(KeyCode::Right));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
            "│▶ Playlists (0):                                          │",
            "│▼ Collections (0):                                        │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = SongView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(60, 12);
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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
            "│▶ Playlists (1):                                          │",
            "│▶ Collections (1):                                        │",
            "│                                                          │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_checked() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SongView::new(&state_with_everything(), tx);
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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☑ Album: Test Album Test Artist                           │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SongView::new(&state_with_everything(), tx);

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
        let mut view = SongView::new(&state_with_everything(), tx);

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
                ("song", item_id()).into()
            ])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("song", item_id()).into()
            ],)))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("song", item_id()).into()
            ])))
        );

        // there are checked items
        // first we need to select an item
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Album(item_id())))
        );

        // check the item
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("album", item_id()).into()
            ])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("album", item_id()).into()
            ],)))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("album", item_id()).into()
            ])))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_mouse_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SongView::new(&state_with_everything(), tx);

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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▶ Artists (1):                                            │",
            "│☐ Album: Test Album Test Artist                           │",
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
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on the song─────────────────────────│",
            "│▼ Artists (1):                                            │",
            "│  ☐ Test Artist                                           │",
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
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Song View─────────────────────────────────────────────────┐",
            "│                   Test Song Test Artist                  │",
            "│  Track/Disc: 0/0  Duration: 3:00.0  Genre(s): Test Genre │",
            "│                                                          │",
            "│q: add to queue | r: start radio | p: add to playlist─────│",
            "│Performing operations on checked items────────────────────│",
            "│▼ Artists (1):                                            │",
            "│  ☑ Test Artist                                           │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);
        // ctrl click on it
        for _ in 0..2 {
            view.handle_mouse_event(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: 2,
                    row: 7,
                    modifiers: KeyModifiers::CONTROL,
                },
                area,
            );
            assert_eq!(
                rx.blocking_recv().unwrap(),
                Action::ActiveView(ViewAction::Set(ActiveView::Artist(item_id())))
            );
        }

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
        let view = LibrarySongsView::new(&state, tx);

        assert_eq!(view.name(), "Library Songs View");
        assert_eq!(view.props.songs, state.library.songs);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = LibrarySongsView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(view.props.songs, new_state.library.songs);
    }

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = LibrarySongsView::new(&state_with_everything(), tx);

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
            "┌Library Songs sorted by: Artist───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_render_with_checked() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibrarySongsView::new(&state_with_everything(), tx);
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
            "┌Library Songs sorted by: Artist───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        // check the first song
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Library Songs sorted by: Artist───────────────────────────┐",
            "│q: add to queue | r: start radio | p: add to playlist ────│",
            "│☑ Test Song Test Artist                                   │",
            "│                                                          │",
            "│s/S: change sort──────────────────────────────────────────│",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check───────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibrarySongsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, SongSort::Artist);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SongSort::Album);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SongSort::AlbumArtist);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SongSort::Genre);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SongSort::Title);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SongSort::Artist);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SongSort::Title);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SongSort::Genre);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SongSort::AlbumArtist);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SongSort::Album);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SongSort::Artist);
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibrarySongsView::new(&state_with_everything(), tx);

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
        let mut view = LibrarySongsView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(60, 9);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        terminal.draw(|frame| view.render(frame, props)).unwrap();

        // first we need to navigate to the song
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
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
        );

        // there are checked items
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                ("song", item_id()).into()
            ])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveView(ViewAction::Set(ActiveView::Radio(vec![
                ("song", item_id()).into()
            ],)))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![
                ("song", item_id()).into()
            ])))
        );
    }

    #[test]
    fn test_mouse() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibrarySongsView::new(&state_with_everything(), tx);

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
            "┌Library Songs sorted by: Artist───────────────────────────┐",
            "│──────────────────────────────────────────────────────────│",
            "│☐ Test Song Test Artist                                   │",
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
            "┌Library Songs sorted by: Artist───────────────────────────┐",
            "│q: add to queue | r: start radio | p: add to playlist ────│",
            "│☑ Test Song Test Artist                                   │",
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

        // ctrl-click down on selected item
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
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(item_id())))
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
