//! Views for both a single song, and a list of songs.

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use mecomp_storage::db::schemas::Thing;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_tree_widget::{Tree, TreeState};

use crate::{
    state::action::{Action, AudioAction, QueueAction},
    ui::{
        components::{Component, ComponentRender, RenderProps},
        AppState,
    },
};

use super::{
    none::NoneView,
    utils::{create_album_tree_leaf, create_artist_tree_item},
    SongViewProps,
};

#[allow(clippy::module_name_repetitions)]
pub struct SongView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<SongViewProps>,
    /// tree state
    tree_state: Mutex<TreeState<String>>,
}

impl Component for SongView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.song.clone(),
            tree_state: Mutex::new(TreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.song {
            Self {
                props: Some(props.to_owned()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Song View"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
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
            // Enter key opens selected view
            KeyCode::Enter => {
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
            // Add song to queue
            KeyCode::Char('q') => {
                if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(vec![
                            props.id.clone(),
                        ]))))
                        .unwrap();
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for SongView {
    #[allow(clippy::too_many_lines)]
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };

        if let Some(state) = &self.props {
            let block = Block::bordered()
                .title_top("Song View")
                .title_bottom("Enter: Open | ←/↑/↓/→: Navigate")
                .border_style(border_style);
            let block_area = block.inner(props.area);
            frame.render_widget(block, props.area);

            // create list to hold song album and artists
            let album_tree = create_album_tree_leaf(&state.album, Some(Span::raw("Album: ")));
            let artist_tree = create_artist_tree_item(state.artists.as_slice()).unwrap();
            let items = &[artist_tree, album_tree];

            let [top, bottom] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(4)])
                .split(block_area)
            else {
                panic!("Failed to split song view area")
            };

            // render the song info
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(state.song.title.to_string(), Style::default().bold()),
                        Span::raw(" "),
                        Span::styled(
                            state
                                .song
                                .artist
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<String>>()
                                .join(", "),
                            Style::default().italic(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Genres: "),
                        Span::styled(
                            state
                                .song
                                .genre
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<String>>()
                                .join(", "),
                            Style::default().italic(),
                        ),
                        Span::raw("  Track/Disc: "),
                        Span::styled(
                            format!(
                                "{}/{}",
                                state.song.track.unwrap_or_default(),
                                state.song.disc.unwrap_or_default()
                            ),
                            Style::default().italic(),
                        ),
                        Span::raw("  Duration: "),
                        Span::styled(
                            format!(
                                "{}:{:04.1}",
                                state.song.runtime.as_secs() / 60,
                                state.song.runtime.as_secs_f32() % 60.0,
                            ),
                            Style::default().italic(),
                        ),
                    ]),
                ])
                .block(
                    Block::new()
                        .borders(Borders::BOTTOM)
                        .title_bottom("q: add to queue")
                        .border_style(border_style),
                )
                .alignment(Alignment::Center),
                top,
            );

            // render the song artists / album
            frame.render_stateful_widget(
                Tree::new(items)
                    .unwrap()
                    .highlight_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    .node_closed_symbol("▸")
                    .node_open_symbol("▾")
                    .node_no_children_symbol("▪"),
                bottom,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            NoneView.render(frame, props);
        }
    }
}
