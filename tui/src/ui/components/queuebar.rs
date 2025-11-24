//! Implementation of the Queue Bar component, a scrollable list of the songs in the queue.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_core::state::RepeatMode;
use mecomp_storage::db::schemas::song::SongBrief;
use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::{
        action::{Action, AudioAction, ComponentAction, QueueAction},
        component::ActiveComponent,
    },
    ui::colors::{TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL, border_color},
};

use super::{AppState, Component, ComponentRender, RenderProps};

pub struct QueueBar {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub(crate) props: Props,
    /// list state
    list_state: ListState,
}

pub struct Props {
    pub(crate) queue: Box<[SongBrief]>,
    pub(crate) current_position: Option<usize>,
    pub(crate) repeat_mode: RepeatMode,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        Self {
            current_position: value.audio.queue_position,
            repeat_mode: value.audio.repeat_mode.into(),
            queue: value.audio.queue.clone(),
        }
    }
}

impl Component for QueueBar {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let props = Props::from(state);
        Self {
            action_tx,
            list_state: ListState::default().with_selected(props.current_position),
            props,
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let old_current_index = self.props.current_position;
        let new_current_index = state.audio.queue_position;

        let list_state = if old_current_index == new_current_index {
            self.list_state
        } else {
            self.list_state.with_selected(new_current_index)
        };

        Self {
            list_state,
            props: Props::from(state),
            ..self
        }
    }

    fn name(&self) -> &'static str {
        "Queue"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // Move the selected index up
            KeyCode::Up => {
                if let Some(index) = self.list_state.selected() {
                    let new_index = if index == 0 {
                        self.props.queue.len() - 1
                    } else {
                        index - 1
                    };
                    self.list_state.select(Some(new_index));
                }
            }
            // Move the selected index down
            KeyCode::Down => {
                if let Some(index) = self.list_state.selected() {
                    let new_index = if index == self.props.queue.len() - 1 {
                        0
                    } else {
                        index + 1
                    };
                    self.list_state.select(Some(new_index));
                }
            }
            // Set the current song to the selected index
            KeyCode::Enter => {
                if let Some(index) = self.list_state.selected() {
                    #[allow(clippy::cast_possible_truncation)]
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::SetPosition(
                            index as u64,
                        ))))
                        .unwrap();
                }
            }
            // Clear the queue
            KeyCode::Char('c') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Queue(QueueAction::Clear)))
                    .unwrap();
            }
            // Remove the selected index from the queue
            KeyCode::Char('d') => {
                if let Some(index) = self.list_state.selected() {
                    #[allow(clippy::cast_possible_truncation)]
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Remove(
                            index as u64,
                        ))))
                        .unwrap();
                }
            }
            // shuffle the queue
            KeyCode::Char('s') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Queue(QueueAction::Shuffle)))
                    .unwrap();
            }
            // set the repeat mode
            KeyCode::Char('r') => {
                let repeat_mode = self.props.repeat_mode.into();
                self.action_tx
                    .send(Action::Audio(AudioAction::Queue(
                        QueueAction::SetRepeatMode(repeat_mode),
                    )))
                    .unwrap();
            }
            _ => {}
        }
    }

    // TODO: refactor QueueBar to use a CheckTree for better mouse handling
    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        match kind {
            // TODO: refactor Sidebar to use a CheckTree for better mouse handling
            MouseEventKind::Down(MouseButton::Left) if area.contains(mouse_position) => {
                // make this the active component
                self.action_tx
                    .send(Action::ActiveComponent(ComponentAction::Set(
                        ActiveComponent::QueueBar,
                    )))
                    .unwrap();

                // TODO: when we have better mouse handling, we can use this to select an item
            }
            MouseEventKind::ScrollDown => self.handle_key_event(KeyEvent::from(KeyCode::Down)),
            MouseEventKind::ScrollUp => self.handle_key_event(KeyEvent::from(KeyCode::Up)),
            _ => {}
        }
    }
}

fn split_area(area: Rect) -> [Rect; 3] {
    let [info_area, content_area, instructions_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(area)
    else {
        panic!("Failed to split queue bar area")
    };
    [info_area, content_area, instructions_area]
}

impl ComponentRender<RenderProps> for QueueBar {
    fn render_border(&self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        let border = Block::bordered().title("Queue").border_style(border_style);
        frame.render_widget(&border, props.area);
        let area = border.inner(props.area);

        // split up area
        let [info_area, content_area, instructions_area] = split_area(area);

        // border the content area
        let border = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .title(format!("Songs ({})", self.props.queue.len()))
            .border_style(border_style);
        frame.render_widget(&border, content_area);
        let content_area = border.inner(content_area);

        // render queue info chunk
        let queue_info = format!(
            "repeat: {}",
            self.props.repeat_mode.to_string().to_lowercase()
        );
        frame.render_widget(
            Paragraph::new(queue_info)
                .style(Style::default().fg((*TEXT_NORMAL).into()))
                .alignment(ratatui::layout::Alignment::Center),
            info_area,
        );

        // render instructions
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from("↑/↓: Move | c: Clear"),
                Line::from("\u{23CE} : Select | d: Delete"),
                Line::from("s: Shuffle | r: Repeat"),
            ]))
            .style(Style::default().fg((*TEXT_NORMAL).into()))
            .alignment(ratatui::layout::Alignment::Center),
            instructions_area,
        );

        // return the new props
        RenderProps {
            area: content_area,
            is_focused: props.is_focused,
        }
    }

    fn render_content(&self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
        let items = self
            .props
            .queue
            .iter()
            .enumerate()
            .map(|(index, song)| {
                let style = if Some(index) == self.props.current_position {
                    Style::default().fg((*TEXT_HIGHLIGHT_ALT).into())
                } else {
                    Style::default().fg((*TEXT_NORMAL).into())
                };

                ListItem::new(song.title.as_str()).style(style)
            })
            .collect::<Vec<_>>();

        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(
                    Style::default()
                        .fg((*TEXT_HIGHLIGHT).into())
                        .add_modifier(Modifier::BOLD),
                )
                .scroll_padding(1)
                .direction(ratatui::widgets::ListDirection::TopToBottom),
            props.area,
            &mut self.list_state.clone(),
        );
    }
}
