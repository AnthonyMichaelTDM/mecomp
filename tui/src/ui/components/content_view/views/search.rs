//! implementation the search view

use std::sync::Mutex;

use crossterm::event::KeyCode;
use mecomp_core::rpc::SearchResult;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction},
    ui::{
        colors::{
            BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL,
        },
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        widgets::{
            input_box::{self, InputBox},
            popups::PopupType,
            tree::{state::CheckTreeState, CheckTree},
        },
        AppState,
    },
};

use super::{
    checktree_utils::{
        create_album_tree_item, create_artist_tree_item, create_song_tree_item,
        get_checked_things_from_tree_state, get_selected_things_from_tree_state,
    },
    RADIO_SIZE,
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
            ..self
        }
    }

    fn name(&self) -> &str {
        "Search"
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(self.props.search_results.len().saturating_sub(1), |c| {
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
            // when searchbar focused, enter key will search
            KeyCode::Enter if self.search_bar_focused => {
                self.search_bar_focused = false;
                self.tree_state.lock().unwrap().close_all();
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
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // when search bar unfocused, and there are checked items, "q" will send the checked items to the queue
            KeyCode::Char('q') if !self.search_bar_focused => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
                        .unwrap();
                }
            }
            // when search bar unfocused, and there are checked items, "r" will start a radio with the checked items
            KeyCode::Char('r') if !self.search_bar_focused => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::SetCurrentView(ActiveView::Radio(
                            things, RADIO_SIZE,
                        )))
                        .unwrap();
                }
            }
            // when search bar unfocused, and there are checked items, "p" will send the checked items to the playlist
            KeyCode::Char('p') if !self.search_bar_focused => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
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
}

impl ComponentRender<RenderProps> for SearchView {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // split view
        let [search_bar_area, content_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)].as_ref())
            .split(props.area)
        else {
            panic!("Failed to split search view area");
        };

        // render the search bar
        self.search_bar.render(
            frame,
            input_box::RenderProps {
                area: search_bar_area,
                text_color: if self.search_bar_focused {
                    TEXT_HIGHLIGHT_ALT.into()
                } else {
                    TEXT_NORMAL.into()
                },
                border: Block::bordered()
                    .title("Search")
                    .border_style(Style::default().fg(
                        if self.search_bar_focused && props.is_focused {
                            BORDER_FOCUSED.into()
                        } else {
                            BORDER_UNFOCUSED.into()
                        },
                    )),
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
        let area =
            if get_checked_things_from_tree_state(&self.tree_state.lock().unwrap()).is_empty() {
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

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        // create tree to hold results
        let song_tree = create_song_tree_item(&self.props.search_results.songs).unwrap();
        let album_tree = create_album_tree_item(&self.props.search_results.albums).unwrap();
        let artist_tree = create_artist_tree_item(&self.props.search_results.artists).unwrap();
        let items = &[song_tree, album_tree, artist_tree];

        // render the search results
        frame.render_stateful_widget(
            CheckTree::new(items)
                .unwrap()
                .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
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
    use anyhow::Result;
    use crossterm::event::KeyEvent;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_render_search_focused() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let mut terminal = setup_test_terminal(24, 8);
        let area = terminal.size()?;
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

        Ok(())
    }

    #[test]
    fn test_render_search_unfocused() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let mut terminal = setup_test_terminal(32, 9);
        let area = terminal.size()?;
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

        Ok(())
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
    fn test_keys() -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = SearchView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::Search,
            ..state_with_everything()
        });

        let mut terminal = setup_test_terminal(32, 10);
        let area = terminal.size()?;
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
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "song",
                item_id()
            )
                .into()])))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::SetCurrentView(ActiveView::Radio(
                vec![("song", item_id()).into()],
                RADIO_SIZE
            ))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "song",
                item_id()
            )
                .into()])))
        );

        Ok(())
    }
}
