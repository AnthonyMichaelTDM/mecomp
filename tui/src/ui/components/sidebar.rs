//! Implement the sidebar component.
//!
//! Responsible for allowing users to navigate between different `ContentViews`.

use std::fmt::Display;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_NORMAL},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub enum SidebarItem {
    Search,
    Songs,
    Artists,
    Albums,
    Playlists,
    Collections,
    Space, // this is used to create space between the library actions and the other items
    LibraryRescan,
    LibraryAnalyze,
    LibraryRecluster,
}

impl Display for SidebarItem {
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
            Self::Space => write!(f, ""),
            Self::LibraryRecluster => write!(f, "Library Recluster"),
        }
    }
}

const SIDEBAR_ITEMS: [SidebarItem; 11] = [
    SidebarItem::Search,
    SidebarItem::Space,
    SidebarItem::Songs,
    SidebarItem::Artists,
    SidebarItem::Albums,
    SidebarItem::Playlists,
    SidebarItem::Collections,
    SidebarItem::Space,
    SidebarItem::LibraryRescan,
    SidebarItem::LibraryAnalyze,
    SidebarItem::LibraryRecluster,
];

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
                        SIDEBAR_ITEMS.len() - 1
                    } else {
                        selected - 1
                    };
                    self.list_state.select(Some(new_selected));
                } else {
                    self.list_state.select(Some(SIDEBAR_ITEMS.len() - 1));
                }
            }
            // move the selected index down
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = if selected == SIDEBAR_ITEMS.len() - 1 {
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
                    let item = SIDEBAR_ITEMS[selected];
                    match item {
                        SidebarItem::Search => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Search))
                            .unwrap(),
                        SidebarItem::LibraryRescan => self
                            .action_tx
                            .send(Action::Library(LibraryAction::Rescan))
                            .unwrap(),
                        SidebarItem::LibraryAnalyze => self
                            .action_tx
                            .send(Action::Library(LibraryAction::Analyze))
                            .unwrap(),
                        SidebarItem::LibraryRecluster => self
                            .action_tx
                            .send(Action::Library(LibraryAction::Recluster))
                            .unwrap(),
                        SidebarItem::Songs => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Songs))
                            .unwrap(),
                        SidebarItem::Artists => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Artists))
                            .unwrap(),
                        SidebarItem::Albums => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Albums))
                            .unwrap(),
                        SidebarItem::Playlists => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Playlists))
                            .unwrap(),
                        SidebarItem::Collections => self
                            .action_tx
                            .send(Action::SetCurrentView(ActiveView::Collections))
                            .unwrap(),
                        SidebarItem::Space => {}
                    }
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for Sidebar {
    fn render_border(&self, frame: &mut Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let border = Block::bordered()
            .title_top("Sidebar")
            .title_bottom(Line::from("Enter: Select").alignment(Alignment::Center))
            .border_style(border_style);
        frame.render_widget(&border, props.area);
        let area = border.inner(props.area);
        let border = Block::default()
            .borders(Borders::BOTTOM)
            .title_bottom(Line::from("↑/↓: Move").alignment(Alignment::Center))
            .border_style(border_style);
        frame.render_widget(&border, area);
        let area = border.inner(area);
        RenderProps {
            area,
            is_focused: props.is_focused,
        }
    }

    fn render_content(&self, frame: &mut Frame, props: RenderProps) {
        let items = SIDEBAR_ITEMS
            .iter()
            .map(|item| {
                ListItem::new(Span::styled(
                    item.to_string(),
                    Style::default().fg(TEXT_NORMAL.into()),
                ))
            })
            .collect::<Vec<_>>();

        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(TEXT_HIGHLIGHT.into())
                        .add_modifier(Modifier::BOLD),
                )
                .direction(ratatui::widgets::ListDirection::TopToBottom),
            props.area,
            &mut self.list_state.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, setup_test_terminal, state_with_everything},
        ui::app::ActiveComponent,
    };

    #[test]
    fn test_sidebar_item_display() {
        assert_eq!(SidebarItem::Search.to_string(), "Search");
        assert_eq!(SidebarItem::LibraryRescan.to_string(), "Library Rescan");
        assert_eq!(SidebarItem::LibraryAnalyze.to_string(), "Library Analyze");
        assert_eq!(SidebarItem::Songs.to_string(), "Songs");
        assert_eq!(SidebarItem::Artists.to_string(), "Artists");
        assert_eq!(SidebarItem::Albums.to_string(), "Albums");
        assert_eq!(SidebarItem::Playlists.to_string(), "Playlists");
        assert_eq!(SidebarItem::Collections.to_string(), "Collections");
        assert_eq!(SidebarItem::Space.to_string(), "");
        assert_eq!(
            SidebarItem::LibraryRecluster.to_string(),
            "Library Recluster"
        );
    }

    #[test]
    fn test_sidebar_render() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let sidebar = Sidebar::new(&AppState::default(), tx).move_with_state(&AppState {
            active_component: ActiveComponent::Sidebar,
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(19, 14);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal.draw(|frame| sidebar.render(frame, props))?.buffer;
        let expected = Buffer::with_lines([
            "┌Sidebar──────────┐",
            "│Search           │",
            "│                 │",
            "│Songs            │",
            "│Artists          │",
            "│Albums           │",
            "│Playlists        │",
            "│Collections      │",
            "│                 │",
            "│Library Rescan   │",
            "│Library Analyze  │",
            "│Library Recluster│",
            "│────↑/↓: Move────│",
            "└──Enter: Select──┘",
        ]);

        assert_buffer_eq(buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_navigation_wraps() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut sidebar = Sidebar::new(&AppState::default(), tx).move_with_state(&AppState {
            active_component: ActiveComponent::Sidebar,
            ..state_with_everything()
        });

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Up));
        assert_eq!(sidebar.list_state.selected(), Some(SIDEBAR_ITEMS.len() - 1));

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(sidebar.list_state.selected(), Some(0));
    }

    #[test]
    fn test_actions() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut sidebar = Sidebar::new(&AppState::default(), tx).move_with_state(&AppState {
            active_component: ActiveComponent::Sidebar,
            ..state_with_everything()
        });

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Search)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Songs)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Artists)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Albums)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Playlists)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Collections)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Rescan)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Analyze)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Recluster)
        );
    }
}
