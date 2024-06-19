//! implementation the search view

use crossterm::event::KeyCode;
use mecomp_core::rpc::SearchResult;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, List, ListItem, ListState},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::Action,
    ui::{
        components::{Component, ComponentRender, RenderProps},
        widgets::searchbar::{self, SearchBar},
        AppState,
    },
};

pub struct SearchView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Props,
    /// list state
    list_state: ListState,
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
        SearchView {
            search_bar: SearchBar::new(state, action_tx.clone()),
            search_bar_focused: true,
            list_state: ListState::default(),
            action_tx,
            props,
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        SearchView {
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
            // move the selected index up
            KeyCode::Up => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = if selected == 0 {
                        self.props.search_results.len() + 3 - 1
                    } else {
                        selected - 1
                    };
                    self.list_state.select(Some(new_selected));
                } else if self.props.search_results.len() > 0 {
                    self.list_state
                        .select(Some(self.props.search_results.len() + 3 - 1));
                } else {
                    self.list_state.select(None);
                }
            }
            // move the selected index down
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = if selected == self.props.search_results.len() + 3 - 1 {
                        0
                    } else {
                        selected + 1
                    };
                    self.list_state.select(Some(new_selected));
                } else if self.props.search_results.len() > 0 {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
            // focus / unfocus the search bar
            KeyCode::Enter if self.search_bar_focused => {
                self.search_bar_focused = false;
                self.list_state.select(None);
                if !self.search_bar.is_empty() {
                    self.action_tx
                        .send(Action::Search(self.search_bar.text().to_string()))
                        .unwrap();
                }
            }
            KeyCode::Char('/') if !self.search_bar_focused => {
                self.search_bar_focused = true;
            }
            // TODO: when searchbar unfocused, make the enter key open up a menu that let's you decide where to
            // put the selected item

            // defer to the search bar, if it is focused
            _ if self.search_bar_focused => {
                self.search_bar.handle_key_event(key);
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for SearchView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        // create list to hold results
        let mut items = Vec::with_capacity(self.props.search_results.len() + 3);

        // extend with song results
        items.push(
            ListItem::new(format!(
                "Songs ({}):",
                self.props.search_results.songs.len()
            ))
            .style(Style::default()),
        );
        items.extend(self.props.search_results.songs.iter().map(|song| {
            let style = Style::default().fg(Color::Green);
            ListItem::new(format!(
                "- {}\n  by: {}",
                song.title,
                song.artist
                    .iter()
                    .map(|artist| artist.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ))
            .style(style)
        }));
        // extend with album results
        items.push(
            ListItem::new(format!(
                "Albums ({}):",
                self.props.search_results.albums.len()
            ))
            .style(Style::default()),
        );
        items.extend(self.props.search_results.albums.iter().map(|album| {
            let style = Style::default().fg(Color::Yellow).italic();
            ListItem::new(format!(
                "- {}\n  by: {}",
                album.title,
                album
                    .artist
                    .iter()
                    .map(|artist| artist.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ))
            .style(style)
        }));
        // extend with artist results
        items.push(
            ListItem::new(format!(
                "Artists ({}):",
                self.props.search_results.artists.len()
            ))
            .style(Style::default()),
        );
        items.extend(self.props.search_results.artists.iter().map(|artist| {
            let style = Style::default().fg(Color::Blue);
            ListItem::new(format!("- {}", artist.name)).style(style)
        }));

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
            List::new(items)
                .block(
                    Block::bordered()
                        .title_top("Results")
                        .title_bottom("Press / to focus search bar, then enter to search")
                        .border_style(border_style),
                )
                .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .direction(ratatui::widgets::ListDirection::TopToBottom),
            results_area,
            &mut self.list_state.clone(),
        );
    }
}
