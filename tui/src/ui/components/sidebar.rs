//! Implement the sidebar component.
//!
//! Responsible for allowing users to navigate between different `ContentViews`.

use std::fmt::Display;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Layout,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction},
    ui::{
        components::{Component, ComponentRender, RenderProps},
        AppState,
    },
};

use super::content_view::ActiveView;

#[allow(clippy::module_name_repetitions)]
pub struct Sidebar {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// List state
    list_state: ListState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumCount)]
#[allow(clippy::module_name_repetitions)]
pub enum SidebarItems {
    Search,
    LibraryRescan,
    LibraryAnalyze,
    Songs,
    Artists,
    Albums,
    Playlists,
    Collections,
}

impl Display for SidebarItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Search => write!(f, "Search"),
            Self::LibraryRescan => write!(f, "Library Rescan"),
            Self::LibraryAnalyze => write!(f, "Library Analyze"),
            Self::Songs => write!(f, "Songs"),
            Self::Artists => write!(f, "Artists"),
            Self::Albums => write!(f, "Albums"),
            Self::Playlists => write!(f, "Playlists"),
            Self::Collections => write!(f, "Collections"),
        }
    }
}

impl Component for Sidebar {
    fn new(_state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            list_state: ListState::default(),
        }
    }

    fn move_with_state(self, _state: &AppState) -> Self
    where
        Self: Sized,
    {
        self
    }

    fn name(&self) -> &str {
        "Sidebar"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // move the selected index up
            KeyCode::Up => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = if selected == 0 {
                        SidebarItems::COUNT - 1
                    } else {
                        selected - 1
                    };
                    self.list_state.select(Some(new_selected));
                } else {
                    self.list_state.select(Some(SidebarItems::COUNT - 1));
                }
            }
            // move the selected index down
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = if selected == SidebarItems::COUNT - 1 {
                        0
                    } else {
                        selected + 1
                    };
                    self.list_state.select(Some(new_selected));
                } else {
                    self.list_state.select(Some(0));
                }
            }
            // select the current item
            KeyCode::Enter => {
                if let Some(selected) = self.list_state.selected() {
                    let item = SidebarItems::iter().nth(selected).unwrap();
                    match item {
                        SidebarItems::Search => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Search))
                            .unwrap(),
                        SidebarItems::LibraryRescan => self
                            .action_tx
                            .send(Action::Library(LibraryAction::Rescan))
                            .unwrap(),
                        SidebarItems::LibraryAnalyze => self
                            .action_tx
                            .send(Action::Library(LibraryAction::Analyze))
                            .unwrap(),
                        SidebarItems::Songs => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Songs))
                            .unwrap(),
                        SidebarItems::Artists => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Artists))
                            .unwrap(),
                        SidebarItems::Albums => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Albums))
                            .unwrap(),
                        SidebarItems::Playlists => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Playlists))
                            .unwrap(),
                        SidebarItems::Collections => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Collections))
                            .unwrap(),
                    }
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for Sidebar {
    fn render(&self, frame: &mut Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        let items = SidebarItems::iter()
            .map(|item| ListItem::new(item.to_string()))
            .collect::<Vec<_>>();

        let [top, bottom] = *Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                [
                    ratatui::layout::Constraint::Min(4),
                    ratatui::layout::Constraint::Length(2),
                ]
                .as_ref(),
            )
            .split(props.area)
        else {
            panic!("Failed to split frame into areas")
        };

        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                        .title_top("Sidebar")
                        .border_style(border_style),
                )
                .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .direction(ratatui::widgets::ListDirection::TopToBottom),
            top,
            &mut self.list_state.clone(),
        );

        frame.render_widget(
            Block::bordered()
                .title_alignment(ratatui::layout::Alignment::Center)
                .title_top("↑/↓: Move")
                .title_bottom("Enter: Select")
                .border_style(border_style),
            bottom,
        );
    }
}