//! implementation the search view

use std::sync::Mutex;

use crossterm::event::KeyCode;
use mecomp_core::rpc::SearchResult;
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::Action,
    ui::{
        components::{Component, ComponentRender, RenderProps},
        widgets::searchbar::{self, SearchBar},
        AppState,
    },
};

use super::utils::{create_album_tree_item, create_artist_tree_item, create_song_tree_item};

#[allow(clippy::module_name_repetitions)]
pub struct SearchView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Props,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
    /// Search Bar
    search_bar: SearchBar,
    /// Is the search bar focused
    search_bar_focused: bool,
}

pub struct Props {
    search_results: SearchResult,
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
            search_bar: SearchBar::new(state, action_tx.clone()),
            search_bar_focused: true,
            tree_state: Mutex::new(TreeState::default()),
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
            // focus / unfocus the search bar
            KeyCode::Enter if self.search_bar_focused => {
                self.search_bar_focused = false;
                self.tree_state.lock().unwrap().close_all();
                if !self.search_bar.is_empty() {
                    self.action_tx
                        .send(Action::Search(self.search_bar.text().to_string()))
                        .unwrap();
                }
            }
            KeyCode::Char('/') if !self.search_bar_focused => {
                self.search_bar_focused = true;
            }
            // when searchbar unfocused, enter key will open the selected node
            KeyCode::Enter if !self.search_bar_focused => {
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

            // defer to the search bar, if it is focused
            _ if self.search_bar_focused => {
                self.search_bar.handle_key_event(key);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for SearchView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        // create list to hold results
        let song_tree = create_song_tree_item(&self.props.search_results.songs).unwrap();
        let album_tree = create_album_tree_item(&self.props.search_results.albums).unwrap();
        let artist_tree = create_artist_tree_item(&self.props.search_results.artists).unwrap();
        let items = &[song_tree, album_tree, artist_tree];

        let [search_bar_area, results_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)].as_ref())
            .split(props.area)
        else {
            panic!("Failed to split search view area");
        };

        // render the search bar
        self.search_bar.render(
            frame,
            searchbar::RenderProps {
                title: "Search".to_string(),
                area: search_bar_area,
                border_color: if self.search_bar_focused && props.is_focused {
                    Color::LightRed
                } else {
                    Color::White
                },
                show_cursor: self.search_bar_focused,
            },
        );

        // render the search results
        frame.render_stateful_widget(
            Tree::new(items)
                .unwrap()
                .block(
                    Block::bordered()
                        .title_top("Results")
                        .title_bottom(if self.search_bar_focused {
                            "Enter: Search"
                        } else {
                            "/: Search | Enter: Open | ←/↑/↓/→: Navigate"
                        })
                        .border_style(border_style),
                )
                .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .node_closed_symbol("▸")
                .node_open_symbol("▾")
                .node_no_children_symbol("▪")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            results_area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
