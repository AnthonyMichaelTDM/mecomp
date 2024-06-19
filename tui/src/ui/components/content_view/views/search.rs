//! implementation the search view

use std::sync::Mutex;

use crossterm::event::KeyCode;
use mecomp_core::rpc::SearchResult;
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Block,
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeItem, TreeState};

use crate::{
    state::action::{Action, AudioAction, QueueAction},
    ui::{
        components::{Component, ComponentRender, RenderProps},
        widgets::searchbar::{self, SearchBar},
        AppState,
    },
};

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
        // defer to the search box (except for the up and down keys)
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
                    // TODO: instead of just adding to the queue, instead open the view for the selected item
                    let things: Vec<Thing> = self
                        .tree_state
                        .lock()
                        .unwrap()
                        .selected()
                        .iter()
                        .filter_map(|id| id.parse::<Thing>().ok())
                        .collect();
                    if !things.is_empty() {
                        self.action_tx
                            .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
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
        let song_tree = TreeItem::new(
            "Songs".to_string(),
            format!("Songs ({}):", self.props.search_results.songs.len()),
            self.props
                .search_results
                .songs
                .iter()
                .map(|song| {
                    TreeItem::new_leaf(
                        song.id.to_string(),
                        Line::from(vec![
                            Span::styled(song.title.to_string(), Style::default().bold()),
                            Span::raw(" "),
                            Span::styled(
                                song.artist
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<String>>()
                                    .join(", "),
                                Style::default().italic(),
                            ),
                        ]),
                    )
                })
                .collect(),
        )
        .unwrap();
        let album_tree = TreeItem::new(
            "Albums".to_string(),
            format!("Albums ({}):", self.props.search_results.albums.len()),
            self.props
                .search_results
                .albums
                .iter()
                .map(|album| {
                    TreeItem::new_leaf(
                        album.id.to_string(),
                        Line::from(vec![
                            Span::styled(album.title.to_string(), Style::default().bold()),
                            Span::raw(" "),
                            Span::styled(
                                album
                                    .artist
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<String>>()
                                    .join(", "),
                                Style::default().italic(),
                            ),
                        ]),
                    )
                })
                .collect(),
        )
        .unwrap();
        let artist_tree = TreeItem::new(
            "Artists".to_string(),
            format!("Artists ({}):", self.props.search_results.artists.len()),
            self.props
                .search_results
                .artists
                .iter()
                .map(|artist| {
                    TreeItem::new_leaf(
                        artist.id.to_string(),
                        Line::from(vec![Span::styled(
                            artist.name.to_string(),
                            Style::default().bold(),
                        )]),
                    )
                })
                .collect(),
        )
        .unwrap();
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
                .node_no_children_symbol("▪"),
            results_area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}
