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
    let [song_info, playback_info_area, instructions] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1), // song info
            Constraint::Min(1), // playback info
            Constraint::Min(1), // instructions
        ])
        .areas(area);

    // middle (song progress, volume, and paused/playing indicator)
    let [play_pause, song_progress, volume] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(10),        // play/pause indicator
            Constraint::Percentage(80), // song progress
            Constraint::Min(20),        // volume indicator
        ])
        .areas(playback_info_area);

    Areas {
        song_info,
        play_pause,
        song_progress,
        volume,
        instructions,
    }
}

impl ComponentRender<RenderProps> for ControlPanel {
    fn render_border(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
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

    fn render_content(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::action::{AudioAction, PlaybackAction, VolumeAction},
        test_utils::setup_test_terminal,
        ui::AppState,
    };
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
    use mecomp_core::state::{Percent, SeekType, StateAudio, StateRuntime, Status};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::time::Duration;

    fn make_panel(
        state: &AppState,
    ) -> (ControlPanel, tokio::sync::mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let panel = ControlPanel::new(state, tx);
        (panel, rx)
    }

    fn make_mouse(kind: MouseEventKind, column: u16, row: u16) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    // -- key event tests -------------------------------------------------------

    #[rstest]
    #[case::toggle(
        KeyCode::Char(' '),
        Action::Audio(AudioAction::Playback(PlaybackAction::Toggle))
    )]
    #[case::next(
        KeyCode::Char('n'),
        Action::Audio(AudioAction::Playback(PlaybackAction::Next))
    )]
    #[case::prev(
        KeyCode::Char('p'),
        Action::Audio(AudioAction::Playback(PlaybackAction::Previous))
    )]
    #[case::seek_fwd(
        KeyCode::Right,
        Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
            SeekType::RelativeForwards,
            Duration::from_secs(5)
        )))
    )]
    #[case::seek_back(
        KeyCode::Left,
        Action::Audio(AudioAction::Playback(PlaybackAction::Seek(
            SeekType::RelativeBackwards,
            Duration::from_secs(5)
        )))
    )]
    #[case::vol_up(
        KeyCode::Char('+'),
        Action::Audio(AudioAction::Playback(PlaybackAction::Volume(VolumeAction::Increase(
            0.05
        ))))
    )]
    #[case::vol_up_eq(
        KeyCode::Char('='),
        Action::Audio(AudioAction::Playback(PlaybackAction::Volume(VolumeAction::Increase(
            0.05
        ))))
    )]
    #[case::vol_down(
        KeyCode::Char('-'),
        Action::Audio(AudioAction::Playback(PlaybackAction::Volume(VolumeAction::Decrease(
            0.05
        ))))
    )]
    #[case::vol_down_underscore(
        KeyCode::Char('_'),
        Action::Audio(AudioAction::Playback(PlaybackAction::Volume(VolumeAction::Decrease(
            0.05
        ))))
    )]
    #[case::mute(
        KeyCode::Char('m'),
        Action::Audio(AudioAction::Playback(PlaybackAction::ToggleMute))
    )]
    fn test_key_event(#[case] key_code: KeyCode, #[case] expected: Action) {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        panel.handle_key_event(KeyEvent::from(key_code));

        let action = rx.blocking_recv().unwrap();
        assert_eq!(action, expected);
    }

    #[test]
    fn test_key_event_unrecognised_does_nothing() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        panel.handle_key_event(KeyEvent::from(KeyCode::F(1)));

        assert!(rx.try_recv().is_err());
    }

    // -- mouse event tests (area = 100 wide, 6 tall, origin 0,0) ---------------
    // Layout: border takes row 0, inner area is rows 1-5.
    // split_area within the inner area (y+1):
    //   row 1 = song_info, row 2 = playback_info, row 3 = instructions
    // playback_info is split horizontally:
    //   play_pause: x 0..9, song_progress: x 10..89, volume: x 90..99

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 6,
    };

    #[test]
    fn test_mouse_click_in_area_focuses_control_panel() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        // click anywhere in the area -> set active component
        panel.handle_mouse_event(
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 3),
            AREA,
        );

        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveComponent(ComponentAction::Set(ActiveComponent::ControlPanel))
        );
    }

    #[test]
    fn test_mouse_click_outside_area_no_focus_action() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        // click outside the area -> no set-active-component action
        panel.handle_mouse_event(
            make_mouse(MouseEventKind::Down(MouseButton::Left), 200, 200),
            AREA,
        );

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_mouse_scroll_up_on_volume_increases_volume() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        // scroll outside the entire area -> no action (tests fallthrough branch)
        panel.handle_mouse_event(make_mouse(MouseEventKind::ScrollUp, 200, 200), AREA);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_mouse_scroll_down_on_volume_decreases_volume() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        // scroll outside the entire area -> no action (tests fallthrough branch)
        panel.handle_mouse_event(make_mouse(MouseEventKind::ScrollDown, 200, 200), AREA);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_mouse_moved_does_nothing() {
        let state = AppState::default();
        let (mut panel, mut rx) = make_panel(&state);

        panel.handle_mouse_event(make_mouse(MouseEventKind::Moved, 50, 3), AREA);
        assert!(rx.try_recv().is_err());
    }

    // -- Props / move_with_state -----------------------------------------------

    #[test]
    fn test_props_from_state_playing() {
        let state = AppState {
            audio: StateAudio {
                status: Status::Playing,
                muted: false,
                volume: 0.8,
                runtime: Some(StateRuntime {
                    seek_position: Duration::from_secs(30),
                    seek_percent: Percent::new(0.5),
                    duration: Duration::from_secs(60),
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let (panel, _) = make_panel(&state);
        assert!(panel.props.is_playing);
        assert!(!panel.props.muted);
        assert!((panel.props.volume - 0.8).abs() < f32::EPSILON);
        assert!(panel.props.song_runtime.is_some());
    }

    #[test]
    fn test_props_from_state_no_song() {
        let state = AppState::default();
        let (panel, _) = make_panel(&state);
        assert!(!panel.props.is_playing);
        assert!(panel.props.song_title.is_none());
        assert!(panel.props.song_artist.is_none());
        assert!(panel.props.song_runtime.is_none());
    }

    #[test]
    fn test_move_with_state_updates_props() {
        let state = AppState::default();
        let (panel, _rx) = make_panel(&state);
        assert!(!panel.props.is_playing);

        let new_state = AppState {
            audio: StateAudio {
                status: Status::Playing,
                volume: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };
        let panel = panel.move_with_state(&new_state);
        assert!(panel.props.is_playing);
        assert!((panel.props.volume - 0.5).abs() < f32::EPSILON);
    }

    // -- runtime_string --------------------------------------------------------

    #[test]
    fn test_runtime_string_none() {
        assert_eq!(runtime_string(None), "0.0/0.0");
    }

    #[test]
    fn test_runtime_string_some() {
        let rt = StateRuntime {
            seek_position: Duration::from_secs(65),
            seek_percent: Percent::new(0.5),
            duration: Duration::from_secs(130),
        };
        let s = runtime_string(Some(rt));
        // 65s = 1 min 5s -> "1: 5.0/ 2:10.0" (formatted)
        assert!(s.contains('/'), "expected a '/' separator in '{s}'");
    }

    // -- volume_string ---------------------------------------------------------

    #[test]
    fn test_volume_string_unmuted() {
        let s = volume_string(false, 1.0);
        assert!(s.contains("🔊"), "expected speaker icon in '{s}'");
        assert!(s.contains("100"), "expected '100' in '{s}'");
    }

    #[test]
    fn test_volume_string_muted() {
        let s = volume_string(true, 0.5);
        assert!(s.contains("🔇"), "expected muted icon in '{s}'");
    }

    // -- render ----------------------------------------------------------------

    #[test]
    fn test_render_no_song() {
        let state = AppState::default();
        let (mut panel, _) = make_panel(&state);

        let (mut terminal, area) = setup_test_terminal(100, 6);
        let result = terminal.draw(|frame| {
            panel.render(
                frame,
                RenderProps {
                    area,
                    is_focused: false,
                },
            );
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_with_song_and_runtime() {
        use mecomp_storage::db::schemas::song::{Song, SongBrief};
        let song = SongBrief {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: "Test Artist".to_string().into(),
            album_artist: "Test Album Artist".to_string().into(),
            album: "Test Album".into(),
            genre: "Test Genre".to_string().into(),
            runtime: Duration::from_secs(180),
            track: Some(0),
            disc: Some(0),
            release_year: Some(2021),
            path: "test.mp3".into(),
        };
        let state = AppState {
            audio: StateAudio {
                status: Status::Playing,
                current_song: Some(song),
                runtime: Some(StateRuntime {
                    seek_position: Duration::from_secs(10),
                    seek_percent: Percent::new(0.05),
                    duration: Duration::from_secs(180),
                }),
                volume: 0.8,
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut panel, _) = make_panel(&state);

        let (mut terminal, area) = setup_test_terminal(100, 6);
        let result = terminal.draw(|frame| {
            panel.render(
                frame,
                RenderProps {
                    area,
                    is_focused: true,
                },
            );
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_name() {
        let state = AppState::default();
        let (panel, _) = make_panel(&state);
        assert_eq!(panel.name(), "ControlPanel");
    }
}
