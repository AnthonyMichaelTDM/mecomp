//! The control panel is a fixed height panel at the bottom of the screen that:
//!
//! - displays the current state of the player (playing, paused, stopped, etc.), and
//! - allows users to control the player (play, pause, stop, etc.), volume, etc.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_core::state::{SeekType, StateRuntime, Status};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, LineGauge},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::{
        action::{Action, AudioAction, ComponentAction, PlaybackAction, VolumeAction},
        component::ActiveComponent,
    },
    ui::colors::{GAUGE_FILLED, GAUGE_UNFILLED, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL, border_color},
};

use super::{AppState, Component, ComponentRender, RenderProps};

pub struct ControlPanel {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub(crate) props: Props,
}

pub struct Props {
    pub(crate) is_playing: bool,
    pub(crate) muted: bool,
    pub(crate) volume: f32,
    pub(crate) song_runtime: Option<StateRuntime>,
    pub(crate) song_title: Option<String>,
    pub(crate) song_artist: Option<String>,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        let value = &value.audio;
        Self {
            is_playing: value.status == Status::Playing,
            muted: value.muted,
            volume: value.volume,
            song_runtime: value.runtime,
            song_title: value.current_song.as_ref().map(|song| song.title.clone()),
            song_artist: value
                .current_song
                .as_ref()
                .map(|song| song.artist.as_slice().join(", ")),
        }
    }
}

impl Component for ControlPanel {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: Props::from(state),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: Props::from(state),
            ..self
        }
    }

    fn name(&self) -> &'static str {
        "ControlPanel"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(' ') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Toggle)))
                    .unwrap();
            }
            KeyCode::Char('n') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Next)))
                    .unwrap();
            }
            KeyCode::Char('p') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Previous,
                    )))
                    .unwrap();
            }
            KeyCode::Right => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
                        SeekType::RelativeForwards,
                        Duration::from_secs(5),
                    ))))
                    .unwrap();
            }
            KeyCode::Left => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
                        SeekType::RelativeBackwards,
                        Duration::from_secs(5),
                    ))))
                    .unwrap();
            }
            KeyCode::Char('+' | '=') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Volume(VolumeAction::Increase(0.05)),
                    )))
                    .unwrap();
            }
            KeyCode::Char('-' | '_') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Volume(VolumeAction::Decrease(0.05)),
                    )))
                    .unwrap();
            }
            KeyCode::Char('m') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::ToggleMute,
                    )))
                    .unwrap();
            }
            // ignore other keys
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
                    ActiveComponent::ControlPanel,
                )))
                .unwrap();
        }

        // adjust area to exclude the border
        let area = Rect {
            y: area.y + 1,
            height: area.height - 1,
            ..area
        };

        // split the area into sub-areas
        let Areas {
            play_pause,
            song_progress,
            volume,
            ..
        } = split_area(area);

        // adjust song_progress area to exclude the runtime label
        let runtime_string_len =
            u16::try_from(runtime_string(self.props.song_runtime).len()).unwrap_or(u16::MAX);
        let song_progress = Rect {
            x: song_progress.x + runtime_string_len,
            width: song_progress.width - runtime_string_len,
            ..song_progress
        };
        // adjust play/pause area to only include the icon
        let play_pause = Rect {
            x: play_pause.x + play_pause.width - 3,
            width: 2,
            ..play_pause
        };
        // adjust volume area to only include the label
        let volume = Rect {
            width: u16::try_from(volume_string(self.props.muted, self.props.volume).len())
                .unwrap_or(u16::MAX),
            ..volume
        };

        match kind {
            MouseEventKind::Down(MouseButton::Left) if play_pause.contains(mouse_position) => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Toggle)))
                    .unwrap();
            }
            MouseEventKind::Down(MouseButton::Left) if song_progress.contains(mouse_position) => {
                // calculate the ratio of the click position to the song progress bar
                let ratio =
                    f64::from(mouse_position.x - song_progress.x) / f64::from(song_progress.width);
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
                        SeekType::Absolute,
                        Duration::from_secs_f64(
                            self.props
                                .song_runtime
                                .map_or(0.0, |runtime| runtime.duration.as_secs_f64())
                                * ratio,
                        ),
                    ))))
                    .unwrap();
            }
            MouseEventKind::Down(MouseButton::Left) if volume.contains(mouse_position) => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::ToggleMute,
                    )))
                    .unwrap();
            }
            MouseEventKind::ScrollUp if volume.contains(mouse_position) => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Volume(VolumeAction::Increase(0.05)),
                    )))
                    .unwrap();
            }
            MouseEventKind::ScrollDown if volume.contains(mouse_position) => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Volume(VolumeAction::Decrease(0.05)),
                    )))
                    .unwrap();
            }
            _ => {}
        }
    }
}

fn runtime_string(runtime: Option<StateRuntime>) -> String {
    runtime.map_or_else(
        || String::from("0.0/0.0"),
        |runtime| {
            format!(
                "{}:{:04.1}/{}:{:04.1}",
                runtime.seek_position.as_secs() / 60,
                runtime.seek_position.as_secs_f32() % 60.0,
                runtime.duration.as_secs() / 60,
                runtime.duration.as_secs_f32() % 60.0
            )
        },
    )
}

fn volume_string(muted: bool, volume: f32) -> String {
    format!(" {}: {:.1}", if muted { "🔇" } else { "🔊" }, volume * 100.)
}

#[derive(Debug)]
struct Areas {
    song_info: Rect,
    play_pause: Rect,
    song_progress: Rect,
    volume: Rect,
    instructions: Rect,
}

fn split_area(area: Rect) -> Areas {
    let [song_info, playback_info_area, instructions] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .split(area)
    else {
        panic!("Failed to split frame into areas");
    };

    // middle (song progress, volume, and paused/playing indicator)
    let [play_pause, song_progress, volume] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),  // play/pause indicator
            Constraint::Max(300), // song progress
            Constraint::Min(20),  // volume indicator
        ])
        .split(playback_info_area)
    else {
        panic!("Failed to split frame into areas");
    };

    Areas {
        song_info,
        play_pause,
        song_progress,
        volume,
        instructions,
    }
}

impl ComponentRender<RenderProps> for ControlPanel {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        let block = Block::new()
            .borders(Borders::TOP)
            .border_style(border_style);
        let block_area = block.inner(props.area);
        frame.render_widget(block, props.area);

        RenderProps {
            area: block_area,
            ..props
        }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let Areas {
            song_info,
            play_pause,
            song_progress,
            volume,
            instructions,
        } = split_area(props.area);

        // top (song title and artist)
        let song_info_widget = self.props.song_title.clone().map_or_else(
            || {
                Line::from("No Song Playing")
                    .style(Style::default().bold().fg((*TEXT_NORMAL).into()))
                    .centered()
            },
            |song_title| {
                Line::from(vec![
                    Span::styled(
                        song_title,
                        Style::default().bold().fg((*TEXT_HIGHLIGHT_ALT).into()),
                    ),
                    Span::raw("   "),
                    Span::styled(
                        self.props.song_artist.clone().unwrap_or_default(),
                        Style::default().italic().fg((*TEXT_NORMAL).into()),
                    ),
                ])
                .centered()
            },
        );

        frame.render_widget(song_info_widget, song_info);

        // middle (song progress, volume, and paused/playing indicator)
        // play/pause indicator
        let play_pause_indicator = if self.props.is_playing {
            "\u{23f8} " // pause symbol
        } else {
            "\u{23f5} " // play symbol
        };
        frame.render_widget(
            Line::from(play_pause_indicator)
                .bold()
                .alignment(Alignment::Right),
            play_pause,
        );

        // song progress
        frame.render_widget(
            LineGauge::default()
                .label(Line::from(runtime_string(self.props.song_runtime)))
                .filled_style(Style::default().fg((*GAUGE_FILLED).into()).bold())
                .unfilled_style(Style::default().fg((*GAUGE_UNFILLED).into()).bold())
                .ratio(self.props.song_runtime.map_or(0.0, |runtime| {
                    (runtime.seek_position.as_secs_f64() / runtime.duration.as_secs_f64())
                        .clamp(0.0, 1.0)
                })),
            song_progress,
        );

        // volume indicator
        frame.render_widget(
            // muted icon if muted, otherwise a volume icon.
            Line::from(volume_string(self.props.muted, self.props.volume))
                .style(Style::default().bold().fg((*TEXT_NORMAL).into()))
                .alignment(Alignment::Left),
            volume,
        );

        // bottom (instructions)
        frame.render_widget(
            Line::from(
                "n/p: next/previous | \u{2423}: play/pause | m: mute | +/-: volume | ←/→: seek",
            )
            .italic()
            .alignment(Alignment::Center),
            instructions,
        );
    }
}
