//! implementation of the radio view

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use super::{
    none::NoneView,
    utils::{create_song_tree_leaf, get_selected_things_from_tree_state},
    RadioViewProps,
};
use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT},
        components::{Component, ComponentRender, RenderProps},
        widgets::popups::PopupType,
        AppState,
    },
};

#[allow(clippy::module_name_repetitions)]
pub struct RadioView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<RadioViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

impl Component for RadioView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.radio.clone(),
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.radio {
            Self {
                props: Some(props.to_owned()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Radio"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(
                        self.props
                            .as_ref()
                            .map_or(0, |p| p.songs.len().saturating_sub(1)),
                        |c| c.saturating_sub(10),
                    )
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
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if !things.is_empty() {
                        debug_assert!(things.len() == 1);
                        let thing = things[0].clone();
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // send radio to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(
                            props.songs.iter().map(|s| s.id.clone().into()).collect(),
                        ))))
                        .expect("failed to send action");
                }
            }
            // add radio to playlist
            KeyCode::Char('p') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(
                            props.songs.iter().map(|s| s.id.clone().into()).collect(),
                        ))))
                        .expect("failed to send action");
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for RadioView {
    fn render(&self, frame: &mut Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Radio", Style::default().bold()),
                    Span::raw(" "),
                    Span::styled(format!("top {}", state.count), Style::default().italic()),
                ]))
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create a list to hold the radio results
            let items = state
                .songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect::<Vec<_>>();

            let [top, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(4)])
                .split(block_area)
            else {
                panic!("Failed to split collection view area")
            };

            // render the instructions
            frame.render_widget(
                Block::new()
                    .borders(Borders::BOTTOM)
                    .title_bottom("q: add to queue | p: add to playlist")
                    .border_style(border_style),
                top,
            );

            // render the radio results
            frame.render_stateful_widget(
                Tree::new(&items)
                    .unwrap()
                    .highlight_style(
                        Style::default()
                            .fg(TEXT_HIGHLIGHT.into())
                            .add_modifier(Modifier::BOLD),
                    )
                    .node_closed_symbol("▸")
                    .node_open_symbol("▾")
                    .node_no_children_symbol("▪")
                    .experimental_scrollbar(Some(Scrollbar::new(
                        ScrollbarOrientation::VerticalRight,
                    ))),
                bottom,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            NoneView.render(frame, props);
        }
    }
}
