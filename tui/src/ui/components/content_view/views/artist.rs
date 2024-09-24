//! Views for both a single artist, and the library of artists.

use std::{fmt::Display, sync::Mutex};

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_core::format_duration;
use mecomp_storage::db::schemas::artist::Artist;
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
        construct_start_radio_action, create_album_tree_item, create_artist_tree_leaf,
        create_song_tree_item, get_checked_things_from_tree_state,
        get_selected_things_from_tree_state,
    },
    ArtistViewProps, RADIO_SIZE,
};

#[allow(clippy::module_name_repetitions)]
pub struct ArtistView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<ArtistViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

impl Component for ArtistView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.artist.clone(),
            tree_state: Mutex::new(CheckTreeState::default()),
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
            // if there are checked items, add them to the queue, otherwise send the whole artist to the queue
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
            // if there are checked items, start radio from checked items, otherwise start radio from artist
            KeyCode::Char('r') => {
                let checked_things =
                    get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if let Some(action) =
                    construct_start_radio_action(checked_things, self.props.as_ref().map(|p| &p.id))
                {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to playlist, otherwise add the whole artist to playlist
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
}

impl ComponentRender<RenderProps> for ArtistView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // draw borders and get content area
        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top("Artist View")
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            let content_area = border.inner(props.area);
            frame.render_widget(border, props.area);

            // split the area to make room for the artist info
            let [info_area, content_area] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(content_area)
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
                            format_duration(&state.artist.runtime),
                            Style::default().italic(),
                        ),
                    ]),
                ])
                .alignment(Alignment::Center),
                info_area,
            );

            // draw an additional border around the content area to display additional instructions
            let border = Block::default()
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
                            "entire artist"
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
                .title_top("Artist View")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create list to hold artist albums and songs
            let album_tree = create_album_tree_item(&state.albums).unwrap();
            let song_tree = create_song_tree_item(&state.songs).unwrap();
            let items = &[album_tree, song_tree];

            // render the artist artists / album
            frame.render_stateful_widget(
                CheckTree::new(items)
                    .unwrap()
                    .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold()),
                props.area,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            let text = "No active artist";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}

pub struct LibraryArtistsView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

struct Props {
    artists: Box<[Artist]>,
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
            tree_state: Mutex::new(CheckTreeState::default()),
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
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

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
                if get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()).is_empty() {
                    ""
                } else {
                    "q: add to queue | r: start radio | p: add to playlist "
                },
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
            .map(|artist| create_artist_tree_leaf(artist))
            .collect::<Vec<_>>();

        // render the artists
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
    fn test_sort_artists() {
        let mut artists = vec![
            Artist {
                id: Artist::generate_id(),
                name: "C".into(),
                song_count: 1,
                album_count: 1,
                runtime: Duration::from_secs(180),
            },
            Artist {
                id: Artist::generate_id(),
                name: "B".into(),
                song_count: 1,
                album_count: 1,
                runtime: Duration::from_secs(180),
            },
            Artist {
                id: Artist::generate_id(),
                name: "A".into(),
                song_count: 1,
                album_count: 1,
                runtime: Duration::from_secs(180),
            },
        ];

        SortMode::Name.sort_artists(&mut artists);
        assert_eq!(artists[0].name, "A".into());
        assert_eq!(artists[1].name, "B".into());
        assert_eq!(artists[2].name, "C".into());
    }
}

#[cfg(test)]
mod item_view_tests {
    use super::*;
    use crate::test_utils::{
        assert_buffer_eq, item_id, setup_test_terminal, state_with_everything,
    };
    use anyhow::Result;
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
    fn test_render_no_artist() -> Result<()> {
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

        Ok(())
    }

    #[test]
    fn test_render() -> Result<()> {
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

        Ok(())
    }

    #[test]
    fn test_render_with_checked() -> Result<()> {
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

        Ok(())
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
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "artist",
                item_id()
            )
                .into()])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Radio(
                vec![("artist", item_id()).into()],
                RADIO_SIZE
            ))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "artist",
                item_id()
            )
                .into()])))
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
            Action::SetCurrentView(ActiveView::Song(item_id()))
        );

        // check the item
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

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Radio(
                vec![("song", item_id()).into()],
                RADIO_SIZE
            ))
        );

        // add to playlist
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
}

#[cfg(test)]
mod library_view_tests {
    use super::*;
    use crate::test_utils::{
        assert_buffer_eq, item_id, setup_test_terminal, state_with_everything,
    };
    use anyhow::Result;
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
    fn test_render() -> Result<()> {
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

        Ok(())
    }

    #[test]
    fn test_render_with_checked() -> Result<()> {
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

        Ok(())
    }

    #[test]
    fn test_sort_keys() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = LibraryArtistsView::new(&state_with_everything(), tx);

        assert_eq!(view.props.sort_mode, SortMode::Name);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(view.props.sort_mode, SortMode::Name);
        view.handle_key_event(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(view.props.sort_mode, SortMode::Name);
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
            Action::SetCurrentView(ActiveView::Artist(item_id()))
        );

        // there are checked items
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "artist",
                item_id()
            )
                .into()])))
        );

        // start radio
        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::SetCurrentView(ActiveView::Radio(
                vec![("artist", item_id()).into()],
                RADIO_SIZE
            ))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "artist",
                item_id()
            )
                .into()])))
        );
    }
}
