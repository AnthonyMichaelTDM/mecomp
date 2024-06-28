//! A popup that prompts the user to select a playlist, or create a new one.
//!
//! The popup will consist of an input box for the playlist name, a list of playlists to select from, and a button to create a new playlist.
//!
//! The user can navigate the list of playlists using the arrow keys, and select a playlist by pressing the enter key.
//!
//! The user can create a new playlist by typing a name in the input box and pressing the enter key.
//!
//! The user can cancel the popup by pressing the escape key.

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction},
    ui::{
        colors::{BORDER_FOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT},
        components::{
            content_view::views::{
                checktree_utils::{create_playlist_tree_leaf, get_selected_things_from_tree_state},
                playlist::Props,
            },
            Component, ComponentRender,
        },
        widgets::{
            input_box::{InputBox, RenderProps},
            tree::{state::CheckTreeState, CheckTree},
        },
        AppState,
    },
};

use super::Popup;

/// A popup that prompts the user to select a playlist, or create a new one.
///
/// The popup will consist of a list of playlists to select from,
/// and if the user wants to create a new playlist, they can press the "n" key,
/// which will make an input box appear for the user to type the name of the new playlist.
#[allow(clippy::module_name_repetitions)]
pub struct PlaylistSelector {
    /// Action Sender
    action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// Playlist Name Input Box
    input_box: InputBox,
    /// Is the input box visible
    input_box_visible: bool,
    /// The items to add to the playlist
    items: Vec<Thing>,
}

impl PlaylistSelector {
    #[must_use]
    pub fn new(state: &AppState, action_tx: UnboundedSender<Action>, items: Vec<Thing>) -> Self {
        Self {
            input_box: InputBox::new(state, action_tx.clone()),
            input_box_visible: false,
            action_tx,
            props: Props::from(state),
            tree_state: Mutex::new(CheckTreeState::default()),
            items,
        }
    }
}

impl Popup for PlaylistSelector {
    fn title(&self) -> ratatui::prelude::Line {
        Line::from("Select a Playlist")
    }

    fn instructions(&self) -> ratatui::prelude::Line {
        Line::from(if self.input_box_visible {
            ""
        } else {
            "  \u{23CE} : Select | ↑/↓: Up/Down"
        })
    }

    fn update_with_state(&mut self, state: &AppState) {
        self.props = Props::from(state);
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        let [_, horizontal_area, _] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(terminal_area)
        else {
            panic!("Failed to split frame size");
        };

        let [_, area, _] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(horizontal_area)
        else {
            panic!("Failed to split horizontal area");
        };
        area
    }

    fn inner_handle_key_event(&mut self, key: KeyEvent) {
        // this component has 2 distinct states:
        // 1. the user is selecting a playlist
        // 2. the user is creating a new playlist
        // when the user is creating a new playlist, the input box is visible
        // and the user can type the name of the new playlist
        // when the user is selecting a playlist, the input box is not visible
        // and the user can navigate the list of playlists
        if self.input_box_visible {
            match key.code {
                // if the user presses Enter, we try to create a new playlist with the given name
                // and add the items to that playlist
                KeyCode::Enter => {
                    let name = self.input_box.text();
                    if !name.is_empty() {
                        // create the playlist and add the items,
                        self.action_tx
                            .send(Action::Library(LibraryAction::CreatePlaylistAndAddThings(
                                name.to_string(),
                                self.items.clone(),
                            )))
                            .unwrap();
                        // close the popup
                        self.action_tx
                            .send(Action::Popup(PopupAction::Close))
                            .unwrap();
                    }
                    self.input_box_visible = false;
                }
                // defer to the input box
                _ => self.input_box.handle_key_event(key),
            }
        } else {
            match key.code {
                // if the user presses the "n" key, we show the input box
                KeyCode::Char('n') => {
                    self.input_box_visible = true;
                }
                // arrow keys
                KeyCode::PageUp => {
                    self.tree_state.lock().unwrap().select_relative(|current| {
                        current.map_or(self.props.playlists.len() - 1, |c| c.saturating_sub(10))
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
                // Enter key adds the items to the selected playlist
                // and closes the popup
                KeyCode::Enter => {
                    if self.tree_state.lock().unwrap().toggle_selected() {
                        let things =
                            get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                        if let Some(thing) = things {
                            // add the items to the selected playlist
                            self.action_tx
                                .send(Action::Library(LibraryAction::AddThingsToPlaylist(
                                    thing,
                                    self.items.clone(),
                                )))
                                .unwrap();
                            // close the popup
                            self.action_tx
                                .send(Action::Popup(PopupAction::Close))
                                .unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl ComponentRender<Rect> for PlaylistSelector {
    fn render_border(&self, frame: &mut ratatui::Frame, area: Rect) -> Rect {
        let area = self.render_popup_border(frame, area);

        let content_area = if self.input_box_visible {
            // split content area to make room for the input box
            let [input_box_area, content_area] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(area)
            else {
                panic!("Failed to split library playlists view area");
            };

            // render input box
            self.input_box.render(
                frame,
                RenderProps {
                    area: input_box_area,
                    text_color: TEXT_HIGHLIGHT_ALT.into(),
                    border: Block::bordered()
                        .title("Enter Name:")
                        .border_style(Style::default().fg(BORDER_FOCUSED.into())),
                    show_cursor: self.input_box_visible,
                },
            );

            content_area
        } else {
            area
        };

        // draw additional border around content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP)
            .title_top(if self.input_box_visible {
                "  \u{23CE} : Create (cancel if empty)"
            } else {
                "n: new playlist"
            })
            .border_style(Style::default().fg(self.border_color()));
        frame.render_widget(&border, content_area);
        border.inner(content_area)
    }

    fn render_content(&self, frame: &mut Frame, area: Rect) {
        // create a tree for the playlists
        let playlists = self
            .props
            .playlists
            .iter()
            .map(create_playlist_tree_leaf)
            .collect::<Vec<_>>();

        // render the playlists
        frame.render_stateful_widget(
            CheckTree::new(&playlists)
                .unwrap()
                .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
                // we want this to be rendered like a normal tree, not a check tree, so we don't show the checkboxes
                .node_unselected_symbol("▪ ")
                .node_selected_symbol("▪ ")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
