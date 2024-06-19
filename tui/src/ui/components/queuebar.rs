//! Implementation of the Queue Bar component, a scrollable list of the songs in the queue.

use crossterm::event::KeyCode;
use mecomp_core::state::RepeatMode;
use mecomp_storage::db::schemas::song::Song;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use tokio::sync::mpsc::UnboundedSender;

use crate::state::action::{Action, AudioAction, QueueAction};

use super::{AppState, Component, ComponentRender, RenderProps};

pub struct QueueBar {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// list state
    list_state: ListState,
}

struct Props {
    queue: Box<[Song]>,
    current_position: Option<usize>,
    repeat_mode: RepeatMode,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        Self {
            current_position: value.audio.queue_position,
            repeat_mode: value.audio.repeat_mode,
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

    fn name(&self) -> &str {
        "Queue"
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
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
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::SetPosition(
                            index,
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
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Remove(
                            index,
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
            KeyCode::Char('r') => match self.props.repeat_mode {
                RepeatMode::None => {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(
                            QueueAction::SetRepeatMode(RepeatMode::Once),
                        )))
                        .unwrap();
                }
                RepeatMode::Once => {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(
                            QueueAction::SetRepeatMode(RepeatMode::Continuous),
                        )))
                        .unwrap();
                }
                RepeatMode::Continuous => {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(
                            QueueAction::SetRepeatMode(RepeatMode::None),
                        )))
                        .unwrap();
                }
            },
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for QueueBar {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        let items = self
            .props
            .queue
            .iter()
            .enumerate()
            .map(|(index, song)| {
                let style = if Some(index) == self.props.current_position {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                ListItem::new(song.title.as_ref()).style(style)
            })
            .collect::<Vec<_>>();

        let [top, middle, bottom] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(2),
                    Constraint::Min(0),
                    Constraint::Length(4),
                ]
                .as_ref(),
            )
            .split(props.area)
        else {
            panic!("Failed to split queue bar area");
        };

        // Top (queue info)
        let queue_info = format!(
            "repeat: {}",
            match self.props.repeat_mode {
                RepeatMode::None => "none",
                RepeatMode::Once => "once",
                RepeatMode::Continuous => "continuous",
            }
        );
        frame.render_widget(
            Paragraph::new(queue_info)
                .block(
                    Block::default()
                        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                        .title("Queue")
                        .border_style(border_style),
                )
                .style(Style::default().fg(Color::White))
                .alignment(ratatui::layout::Alignment::Center),
            top,
        );

        // middle (queue list)
        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::bordered()
                        .title(format!("Songs ({})", self.props.queue.len()))
                        .border_style(border_style),
                )
                .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .scroll_padding(1)
                .direction(ratatui::widgets::ListDirection::TopToBottom),
            middle,
            &mut self.list_state.clone(),
        );

        // Bottom (instructions)
        frame.render_widget(
            Paragraph::new(
                "↑/↓: Move | c: Clear\nEnter: Select | d: Delete\ns: Shuffle | r: Repeat",
            )
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                    .border_style(border_style),
            )
            .style(Style::default().fg(Color::White))
            .alignment(ratatui::layout::Alignment::Center),
            bottom,
        );
    }
}
