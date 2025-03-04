//! implementation of the random view

use std::fmt::Display;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Alignment, Margin, Position, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use super::RandomViewProps;
use crate::{
    state::action::{Action, ViewAction},
    ui::{
        colors::{border_color, TEXT_HIGHLIGHT, TEXT_NORMAL},
        components::{content_view::ActiveView, Component, ComponentRender, RenderProps},
        AppState,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// The type of random item to get
pub enum ItemType {
    Album,
    Artist,
    Song,
}

impl ItemType {
    #[must_use]
    pub fn to_action(&self, props: &RandomViewProps) -> Option<Action> {
        match self {
            Self::Album => Some(Action::ActiveView(ViewAction::Set(ActiveView::Album(
                props.album.id.clone(),
            )))),
            Self::Artist => Some(Action::ActiveView(ViewAction::Set(ActiveView::Artist(
                props.artist.id.clone(),
            )))),
            Self::Song => Some(Action::ActiveView(ViewAction::Set(ActiveView::Song(
                props.song.id.clone(),
            )))),
        }
    }
}

impl Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Album => write!(f, "Random Album"),
            Self::Artist => write!(f, "Random Artist"),
            Self::Song => write!(f, "Random Song"),
        }
    }
}

const RANDOM_TYPE_ITEMS: [ItemType; 3] = [ItemType::Album, ItemType::Artist, ItemType::Song];

#[allow(clippy::module_name_repetitions)]
pub struct RandomView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Props for the random view
    pub props: Option<RandomViewProps>,
    /// State of the list that users interact with to a random item of the selected type
    random_type_list: ListState,
}

impl Component for RandomView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.random.clone(),
            random_type_list: ListState::default(),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.random {
            Self {
                props: Some(props.clone()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &'static str {
        "Random"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // Move the selection up
            KeyCode::Up => {
                let new_selection = self
                    .random_type_list
                    .selected()
                    .filter(|selected| *selected > 0)
                    .map_or_else(|| RANDOM_TYPE_ITEMS.len() - 1, |selected| selected - 1);

                self.random_type_list.select(Some(new_selection));
            }
            // Move the selection down
            KeyCode::Down => {
                let new_selection = self
                    .random_type_list
                    .selected()
                    .filter(|selected| *selected < RANDOM_TYPE_ITEMS.len() - 1)
                    .map_or(0, |selected| selected + 1);

                self.random_type_list.select(Some(new_selection));
            }
            // Select the current item
            KeyCode::Enter => {
                if let Some(selected) = self.random_type_list.selected() {
                    if let Some(action) = RANDOM_TYPE_ITEMS.get(selected).and_then(|item| {
                        self.props.as_ref().and_then(|props| item.to_action(props))
                    }) {
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

        // adjust area to exclude the border
        let area = area.inner(Margin::new(1, 1));

        match kind {
            MouseEventKind::Down(MouseButton::Left) if area.contains(mouse_position) => {
                // adjust the mouse position so that it is relative to the area of the list
                let adjusted_mouse_y = mouse_position.y - area.y;

                // select the item at the mouse position
                let selected = adjusted_mouse_y as usize;
                if self.random_type_list.selected() == Some(selected) {
                    self.handle_key_event(KeyEvent::from(KeyCode::Enter));
                } else if selected < RANDOM_TYPE_ITEMS.len() {
                    self.random_type_list.select(Some(selected));
                } else {
                    self.random_type_list.select(None);
                }
            }
            MouseEventKind::ScrollDown => self.handle_key_event(KeyEvent::from(KeyCode::Down)),
            MouseEventKind::ScrollUp => self.handle_key_event(KeyEvent::from(KeyCode::Up)),
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for RandomView {
    fn render_border(&self, frame: &mut Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        let border = Block::bordered()
            .title_top("Random")
            .title_bottom(" \u{23CE} : select | ↑/↓: Move ")
            .border_style(border_style);
        frame.render_widget(&border, props.area);
        let area = border.inner(props.area);

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut Frame, props: RenderProps) {
        if self.props.is_none() {
            frame.render_widget(
                Line::from("Random items unavailable")
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
            return;
        }

        let items = RANDOM_TYPE_ITEMS
            .iter()
            .map(|item| {
                ListItem::new(
                    Span::styled(item.to_string(), Style::default().fg(TEXT_NORMAL.into()))
                        .into_centered_line(),
                )
            })
            .collect::<Vec<_>>();

        frame.render_stateful_widget(
            List::new(items).highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold()),
            props.area,
            &mut self.random_type_list.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use anyhow::Result;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
    use mecomp_storage::db::schemas::{album::Album, artist::Artist, song::Song};
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_random_view_type_to_action() {
        let props = RandomViewProps {
            album: Album::generate_id().into(),
            artist: Artist::generate_id().into(),
            song: Song::generate_id().into(),
        };

        assert_eq!(
            ItemType::Album.to_action(&props),
            Some(Action::ActiveView(ViewAction::Set(ActiveView::Album(
                props.album.id.clone()
            ))))
        );
        assert_eq!(
            ItemType::Artist.to_action(&props),
            Some(Action::ActiveView(ViewAction::Set(ActiveView::Artist(
                props.artist.id.clone()
            ))))
        );
        assert_eq!(
            ItemType::Song.to_action(&props),
            Some(Action::ActiveView(ViewAction::Set(ActiveView::Song(
                props.song.id.clone()
            ))))
        );
    }

    #[test]
    fn test_random_view_type_display() {
        assert_eq!(ItemType::Album.to_string(), "Random Album");
        assert_eq!(ItemType::Artist.to_string(), "Random Artist");
        assert_eq!(ItemType::Song.to_string(), "Random Song");
    }

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = RandomView::new(&state, tx);

        assert_eq!(view.name(), "Random");
        assert!(view.props.is_some());
        assert_eq!(view.props, state.additional_view_data.random);
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = RandomView::new(&state, tx).move_with_state(&new_state);

        assert!(view.props.is_some());
        assert_eq!(view.props, new_state.additional_view_data.random);
    }

    #[test]
    /// Test rendering when there are no items available (e.g., empty library)
    fn test_render_empty() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = RandomView::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(29, 3);
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
            "┌Random─────────────────────┐",
            "│ Random items unavailable  │",
            "└ ⏎ : select | ↑/↓: Move ───┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_render() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = RandomView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(50, 5);
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
            "┌Random──────────────────────────────────────────┐",
            "│                  Random Album                  │",
            "│                 Random Artist                  │",
            "│                  Random Song                   │",
            "└ ⏎ : select | ↑/↓: Move ────────────────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_navigation_wraps() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = RandomView::new(&state_with_everything(), tx);

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        assert_eq!(
            view.random_type_list.selected(),
            Some(RANDOM_TYPE_ITEMS.len() - 1)
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(view.random_type_list.selected(), Some(0));
    }

    #[test]
    fn test_actions() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let mut view = RandomView::new(&state, tx);
        let random_view_props = state.additional_view_data.random.unwrap();

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Album(
                random_view_props.album.id,
            )))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Artist(
                random_view_props.artist.id,
            )))
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(
                ActiveView::Song(random_view_props.song.id,)
            ))
        );
    }

    #[test]
    fn test_mouse() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let mut view = RandomView::new(&state, tx);
        let random_view_props = state.additional_view_data.random.unwrap();
        let view_area = Rect::new(0, 0, 50, 6);

        // select the first item by scrolling down
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 25,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        // click selected item
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Album(
                random_view_props.album.id.clone(),
            )))
        );

        // select the second item by scrolling down
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 25,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        // click selected item
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 2,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Artist(
                random_view_props.artist.id,
            )))
        );

        // select the first item by clicking on it
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        // click selected item
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Album(
                random_view_props.album.id,
            )))
        );

        // select the third item by clicking on it
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 3,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 3,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::ActiveView(ViewAction::Set(ActiveView::Song(random_view_props.song.id)))
        );

        // clicking on nothing should clear the selection
        view.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 25,
                row: 4,
                modifiers: KeyModifiers::empty(),
            },
            view_area,
        );
        assert_eq!(view.random_type_list.selected(), None);
    }
}
