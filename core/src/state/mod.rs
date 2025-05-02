#![allow(clippy::module_name_repetitions)]
pub mod library;
use std::{fmt::Display, time::Duration};

use mecomp_storage::db::schemas::song::SongBrief;
use serde::{Deserialize, Serialize};

use crate::format_duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SeekType {
    Absolute,
    RelativeForwards,
    RelativeBackwards,
}

impl Display for SeekType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absolute => write!(f, "Absolute"),
            Self::RelativeForwards => write!(f, "Forwards"),
            Self::RelativeBackwards => write!(f, "Backwards"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum RepeatMode {
    /// No repeat: after the queue is finished the player stops
    #[default]
    None,
    /// Repeat the current Song: Repeats the current song, otherwise behaves like `RepeatMode::None`
    One,
    /// Repeat the queue Continuously: after going through the queue, the player goes back to the beginning and continues
    All,
}

impl Display for RepeatMode {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::One => write!(f, "One"),
            Self::All => write!(f, "All"),
        }
    }
}

impl RepeatMode {
    #[must_use]
    #[inline]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    #[must_use]
    #[inline]
    pub const fn is_one(&self) -> bool {
        matches!(self, Self::One)
    }

    #[must_use]
    #[inline]
    pub const fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize, Default)]
pub struct Percent(f32);

impl Percent {
    #[must_use]
    #[inline]
    pub const fn new(value: f32) -> Self {
        Self(if value.is_finite() {
            value.clamp(0.0, 100.0)
        } else {
            0.0
        })
    }

    #[must_use]
    #[inline]
    pub const fn into_inner(self) -> f32 {
        self.0
    }
}

impl Display for Percent {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}%", self.into_inner())
    }
}

/// Information about the runtime of the song song
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Default)]
pub struct StateRuntime {
    pub seek_position: Duration,
    pub seek_percent: Percent,
    pub duration: Duration,
}

impl Display for StateRuntime {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateRuntime {{ seek_position: {}, seek_percent: {}, duration: {} }}",
            format_duration(&self.seek_position),
            self.seek_percent,
            format_duration(&self.duration)
        )
    }
}

#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum Status {
    #[default]
    Stopped,
    Paused,
    Playing,
}

impl Display for Status {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paused => write!(f, "Paused"),
            Self::Playing => write!(f, "Playing"),
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct StateAudio {
    pub queue: Box<[SongBrief]>,
    pub queue_position: Option<usize>,
    pub current_song: Option<SongBrief>,
    pub repeat_mode: RepeatMode,
    pub runtime: Option<StateRuntime>,
    pub status: Status,
    pub muted: bool,
    pub volume: f32,
}

impl Display for StateAudio {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateAudio {{ queue: {:?}, queue_position: {}, current_song: {}, repeat_mode: {}, runtime: {}, status: {}, muted: {}, volume: {:.0}% }}",
            self.queue
                .iter()
                .map(|song| song.title.to_string())
                .collect::<Vec<_>>(),
            self.queue_position
                .map_or_else(|| "None".to_string(), |pos| pos.to_string()),
            self.current_song
                .as_ref()
                .map_or_else(|| "None".to_string(), |song| format!("\"{}\"", song.title)),
            self.repeat_mode,
            self.runtime
                .as_ref()
                .map_or_else(|| "None".to_string(), std::string::ToString::to_string),
            self.status,
            self.muted,
            self.volume * 100.0,
        )
    }
}

impl Default for StateAudio {
    /// Should match the defaults assigned to the [`AudioKernel`]
    #[inline]
    fn default() -> Self {
        Self {
            queue: Box::default(),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::default(),
            runtime: None,
            status: Status::default(),
            muted: false,
            volume: 1.0,
        }
    }
}

impl StateAudio {
    #[must_use]
    #[inline]
    pub const fn paused(&self) -> bool {
        !matches!(self.status, Status::Playing)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use mecomp_storage::db::schemas::song::Song;
    use one_or_many::OneOrMany;
    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

    #[test]
    fn test_state_audio_default() {
        let state = StateAudio::default();
        assert_eq!(state.queue.as_ref(), &[]);
        assert_eq!(state.queue_position, None);
        assert_eq!(state.current_song, None);
        assert_eq!(state.repeat_mode, RepeatMode::None);
        assert_eq!(state.runtime, None);
        assert_eq!(state.status, Status::Stopped);
        assert_eq!(state.muted, false);
        assert!(
            f32::EPSILON > (state.volume - 1.0).abs(),
            "{} != 1.0",
            state.volume
        );
    }

    #[rstest]
    #[case::none(RepeatMode::None, [true, false, false])]
    #[case::one(RepeatMode::One, [false, true, false])]
    #[case::all(RepeatMode::All, [false, false, true])]
    fn test_repeat_mode(#[case] mode: RepeatMode, #[case] expected: [bool; 3]) {
        assert_eq!(mode.is_none(), expected[0]);
        assert_eq!(mode.is_one(), expected[1]);
        assert_eq!(mode.is_all(), expected[2]);
    }

    #[rstest]
    #[case::seek_type(SeekType::Absolute, "Absolute")]
    #[case::seek_type(SeekType::RelativeForwards, "Forwards")]
    #[case::seek_type(SeekType::RelativeBackwards, "Backwards")]
    #[case::repeat_mode(RepeatMode::None, "None")]
    #[case::repeat_mode(RepeatMode::One, "One")]
    #[case::repeat_mode(RepeatMode::All, "All")]
    #[case::percent(Percent::new(50.0), "50.00%")]
    #[case::state_runtimme(
        StateRuntime {
            seek_position: Duration::from_secs(3),
            seek_percent: Percent::new(50.0),
            duration: Duration::from_secs(6),
        },
        "StateRuntime { seek_position: 00:00:03.00, seek_percent: 50.00%, duration: 00:00:06.00 }"
    )]
    #[case::state_audio_empty(
        StateAudio {
            queue: Box::new([]),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::None,
            runtime: None,
            status: Status::Paused,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [], queue_position: None, current_song: None, repeat_mode: None, runtime: None, status: Paused, muted: false, volume: 100% }"
    )]
    #[case::state_audio_empty(
        StateAudio {
            queue: Box::new([]),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::None,
            runtime: None,
            status: Status::Paused,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [], queue_position: None, current_song: None, repeat_mode: None, runtime: None, status: Paused, muted: false, volume: 100% }"
    )]
    #[case::state_audio(
        StateAudio {
            queue: Box::new([
                SongBrief {
                    id: Song::generate_id(),
                    title: "Song 1".into(),
                    artist: OneOrMany::None,
                    album_artist: OneOrMany::None,
                    album: "album".into(),
                    genre: OneOrMany::None,
                    runtime: Duration::from_secs(100),
                    track: None,
                    disc: None,
                    release_year: None,
                    extension: "mp3".into(),
                    path: "foo/bar.mp3".into(),
                }
            ]),
            queue_position: Some(1),
            current_song: Some(
                SongBrief {
                    id: Song::generate_id(),
                    title: "Song 1".into(),
                    artist: OneOrMany::None,
                    album_artist: OneOrMany::None,
                    album: "album".into(),
                    genre: OneOrMany::None,
                    runtime: Duration::from_secs(100),
                    track: None,
                    disc: None,
                    release_year: None,
                    extension: "mp3".into(),
                    path: "foo/bar.mp3".into(),
                }
            ),
            repeat_mode: RepeatMode::None,
            runtime: Some(StateRuntime {
                seek_position: Duration::from_secs(20),
                seek_percent: Percent::new(20.0),
                duration: Duration::from_secs(100),
            }),
            status: Status::Playing,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [\"Song 1\"], queue_position: 1, current_song: \"Song 1\", repeat_mode: None, runtime: StateRuntime { seek_position: 00:00:20.00, seek_percent: 20.00%, duration: 00:01:40.00 }, status: Playing, muted: false, volume: 100% }"
    )]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
