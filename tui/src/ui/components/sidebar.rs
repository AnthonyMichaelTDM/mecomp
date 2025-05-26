//! Implement the sidebar component.
//!
//! Responsible for allowing users to navigate between different `ContentViews`.

use std::fmt::Display;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::{
        action::{Action, ComponentAction, LibraryAction, PopupAction, ViewAction},
        component::ActiveComponent,
    },
    ui::{
        AppState,
        colors::{TEXT_HIGHLIGHT, TEXT_NORMAL, border_color},
        components::{Component, ComponentRender, RenderProps},
        widgets::popups::PopupType,
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
    DynamicPlaylists,
    Collections,
    Random,
    Space, // this is used to create space between the library actions and the other items
    LibraryRescan,
    LibraryAnalyze,
    LibraryRecluster,
}

impl SidebarItem {
    #[must_use]
    pub const fn to_action(&self) -> Option<Action> {
        match self {
            Self::Search => Some(Action::ActiveView(ViewAction::Set(ActiveView::Search))),
            Self::Songs => Some(Action::ActiveView(ViewAction::Set(ActiveView::Songs))),
            Self::Artists => Some(Action::ActiveView(ViewAction::Set(ActiveView::Artists))),
            Self::Albums => Some(Action::ActiveView(ViewAction::Set(ActiveView::Albums))),
            Self::Playlists => Some(Action::ActiveView(ViewAction::Set(ActiveView::Playlists))),
            Self::DynamicPlaylists => Some(Action::ActiveView(ViewAction::Set(
                ActiveView::DynamicPlaylists,
            ))),
            Self::Collections => Some(Action::ActiveView(ViewAction::Set(ActiveView::Collections))),
            Self::Random => Some(Action::ActiveView(ViewAction::Set(ActiveView::Random))),
            Self::Space => None,
            Self::LibraryRescan => Some(Action::Library(LibraryAction::Rescan)),
            Self::LibraryAnalyze => Some(Action::Library(LibraryAction::Analyze)),
            Self::LibraryRecluster => Some(Action::Library(LibraryAction::Recluster)),
        }
    }
}

impl Display for SidebarItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Search => write!(f, "Search"),
            Self::Songs => write!(f, "Songs"),
            Self::Artists => write!(f, "Artists"),
            Self::Albums => write!(f, "Albums"),
            Self::Playlists => write!(f, "Playlists"),
            Self::DynamicPlaylists => write!(f, "Dynamic"),
            Self::Collections => write!(f, "Collections"),
            Self::Random => write!(f, "Random"),
            Self::Space => write!(f, ""),
            Self::LibraryRescan => write!(f, "Library Rescan"),
            Self::LibraryAnalyze => write!(f, "Library Analyze"),
            Self::LibraryRecluster => write!(f, "Library Recluster"),
        }
    }
}

const SIDEBAR_ITEMS: [SidebarItem; 13] = [
    SidebarItem::Search,
    SidebarItem::Space,
    SidebarItem::Songs,
    SidebarItem::Artists,
    SidebarItem::Albums,
    SidebarItem::Playlists,
    SidebarItem::DynamicPlaylists,
    SidebarItem::Collections,
    SidebarItem::Random,
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

    fn name(&self) -> &'static str {
        "Sidebar"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // move the selected index up
            KeyCode::Up => {
                let new_selection = self
                    .list_state
                    .selected()
                    .filter(|selected| *selected > 0)
                    .map_or_else(|| SIDEBAR_ITEMS.len() - 1, |selected| selected - 1);

                self.list_state.select(Some(new_selection));
            }
            // move the selected index down
            KeyCode::Down => {
                let new_selection = self
                    .list_state
                    .selected()
                    .filter(|selected| *selected < SIDEBAR_ITEMS.len() - 1)
                    .map_or(0, |selected| selected + 1);

                self.list_state.select(Some(new_selection));
            }
            // select the current item
            KeyCode::Enter => {
                if let Some(selected) = self.list_state.selected() {
                    let item = SIDEBAR_ITEMS[selected];
                    if let Some(action) = item.to_action() {
                        if matches!(
                            item,
                            SidebarItem::LibraryAnalyze
                                | SidebarItem::LibraryRescan
                                | SidebarItem::LibraryRecluster
                        ) {
                            self.action_tx
                                .send(Action::Popup(PopupAction::Open(PopupType::Notification(
                                    format!(" {item} Started ").into(),
                                ))))
                                .unwrap();
                        }

                        self.action_tx.send(action).unwrap();
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        if kind == MouseEventKind::Down(MouseButton::Left) && area.contains(mouse_position) {
            self.action_tx
                .send(Action::ActiveComponent(ComponentAction::Set(
                    ActiveComponent::Sidebar,
                )))
                .unwrap();
        }

        // adjust area to exclude the border
        let area = area.inner(Margin::new(1, 1));

        match kind {
            // TODO: refactor Sidebar to use a CheckTree for better mouse handling
            MouseEventKind::Down(MouseButton::Left) if area.contains(mouse_position) => {
                // adjust the mouse position so that it is relative to the area of the list
                let adjusted_mouse_y = mouse_position.y - area.y;

                // select the item at the mouse position
                let new_selection = adjusted_mouse_y as usize;
                if self.list_state.selected() == Some(new_selection) {
                    self.handle_key_event(KeyEvent::from(KeyCode::Enter));
                } else if new_selection < SIDEBAR_ITEMS.len() {
                    self.list_state.select(Some(new_selection));
                }
            }
            MouseEventKind::ScrollDown => self.handle_key_event(KeyEvent::from(KeyCode::Down)),
            MouseEventKind::ScrollUp => self.handle_key_event(KeyEvent::from(KeyCode::Up)),
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for Sidebar {
    fn render_border(&self, frame: &mut Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

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
                    Style::default().fg((*TEXT_NORMAL).into()),
                ))
            })
            .collect::<Vec<_>>();

        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(
                    Style::default()
                        .fg((*TEXT_HIGHLIGHT).into())
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
        state::component::ActiveComponent,
        test_utils::{assert_buffer_eq, setup_test_terminal, state_with_everything},
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
        assert_eq!(SidebarItem::Random.to_string(), "Random");
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

        let (mut terminal, area) = setup_test_terminal(19, 16);
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
            "│Dynamic          │",
            "│Collections      │",
            "│Random           │",
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
            Action::ActiveView(ViewAction::Set(ActiveView::Search))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Songs))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Artists))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Albums))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Playlists))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::DynamicPlaylists))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Collections))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Random))
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Notification(
                " Library Rescan Started ".into()
            )))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Rescan)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Notification(
                " Library Analyze Started ".into()
            )))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Analyze)
        );

        sidebar.handle_key_event(KeyEvent::from(KeyCode::Down));
        sidebar.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Notification(
                " Library Recluster Started ".into()
            )))
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Library(LibraryAction::Recluster)
        );
    }
}
