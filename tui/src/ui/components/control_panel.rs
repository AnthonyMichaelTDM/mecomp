//! The control panel is a fixed height panel at the bottom of the screen that displays the current state of the player
//! (playing, paused, stopped, etc.) and allows users to control the player (play, pause, stop, etc.), volume, etc.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, MediaKeyCode};
use mecomp_core::state::{SeekType, StateRuntime};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, LineGauge},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, AudioAction, PlaybackAction, VolumeAction},
    ui::colors::{
        BORDER_FOCUSED, BORDER_UNFOCUSED, GAUGE_FILLED, GAUGE_UNFILLED, TEXT_HIGHLIGHT_ALT,
        TEXT_NORMAL,
    },
};

use super::{AppState, Component, ComponentRender, RenderProps};

pub struct ControlPanel {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
}

struct Props {
    is_playing: bool,
    muted: bool,
    volume: f32,
    song_runtime: Option<StateRuntime>,
    song_title: Option<String>,
    song_artist: Option<String>,
}

impl From<&AppState> for Props {
    fn from(value: &AppState) -> Self {
        let value = &value.audio;
        Self {
            is_playing: !value.paused,
            muted: value.muted,
            volume: value.volume,
            song_runtime: value.runtime,
            song_title: value
                .current_song
                .as_ref()
                .map(|song| song.title.to_string()),
            song_artist: value.current_song.as_ref().map(|song| {
                song.artist
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            }),
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

    fn name(&self) -> &str {
        "ControlPanel"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Media(MediaKeyCode::PlayPause | MediaKeyCode::Play | MediaKeyCode::Pause)
            | KeyCode::Char(' ') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Toggle)))
                    .unwrap();
            }
            KeyCode::Media(MediaKeyCode::TrackNext) | KeyCode::Char('n') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Next)))
                    .unwrap();
            }
            KeyCode::Media(MediaKeyCode::TrackPrevious) | KeyCode::Char('p') => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(
                        PlaybackAction::Previous,
                    )))
                    .unwrap();
            }
            KeyCode::Media(MediaKeyCode::FastForward) | KeyCode::Right => {
                self.action_tx
                    .send(Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
                        SeekType::RelativeForwards,
                        Duration::from_secs(5),
                    ))))
                    .unwrap();
            }
            KeyCode::Media(MediaKeyCode::Rewind) | KeyCode::Left => {
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
}

impl ComponentRender<RenderProps> for ControlPanel {
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

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
        let [song_info_area, playback_info_area, instructions_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ])
            .split(props.area)
        else {
            panic!("main layout must have 3 children");
        };

        // top (song title and artist)
        if let Some(song_title) = self.props.song_title.clone() {
            frame.render_widget(
                Line::from(vec![
                    Span::styled(
                        song_title,
                        Style::default().bold().fg(TEXT_HIGHLIGHT_ALT.into()),
                    ),
                    Span::raw("   "),
                    Span::styled(
                        self.props.song_artist.clone().unwrap_or_default(),
                        Style::default().italic().fg(TEXT_NORMAL.into()),
                    ),
                ])
                .centered(),
                song_info_area,
            );
        } else {
            frame.render_widget(
                Line::from("No Song Playing")
                    .style(Style::default().bold().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                song_info_area,
            );
        }

        // middle (song progress, volume, and paused/playing indicator)
        let [play_pause_area, song_progress_area, volume_area] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(20),  // play/pause indicator
                Constraint::Max(300), // song progress
                Constraint::Min(20),  // volume indicator
            ])
            .split(playback_info_area)
        else {
            panic!("middle layout must have 3 children");
        };

        // play/pause indicator
        frame.render_widget(
            Line::from(if self.props.is_playing {
                "❚❚ "
            } else {
                "▶  "
            })
            .bold()
            .alignment(Alignment::Right),
            play_pause_area,
        );

        // song progress
        frame.render_widget(
            LineGauge::default()
                .label(Line::from(self.props.song_runtime.map_or_else(
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
                )))
                .filled_style(Style::default().fg(GAUGE_FILLED.into()).bold())
                .unfilled_style(Style::default().fg(GAUGE_UNFILLED.into()).bold())
                .ratio(self.props.song_runtime.map_or(0.0, |runtime| {
                    runtime.seek_position.as_secs_f64() / runtime.duration.as_secs_f64()
                })),
            song_progress_area,
        );

        // volume indicator
        frame.render_widget(
            // muted icon if muted, otherwise a volume icon.
            Line::from(format!(
                " {}: {:.1}",
                if self.props.muted { "🔇" } else { "🔊" },
                self.props.volume * 100.
            ))
            .style(Style::default().bold().fg(TEXT_NORMAL.into()))
            .alignment(Alignment::Left),
            volume_area,
        );

        // bottom (instructions)
        frame.render_widget(
            Line::from(
                "n/p: next/previous | \u{2423}: play/pause | m: mute | +/-: volume | ←/→: seek",
            )
            .italic()
            .alignment(Alignment::Center),
            instructions_area,
        );
    }
}
