#![allow(clippy::module_name_repetitions)]
pub mod library;
use std::{fmt::Display, time::Duration};

use mecomp_storage::db::schemas::song::Song;
use nutype::nutype;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SeekType {
    Absolute,
    RelativeForwards,
    RelativeBackwards,
}

impl Display for SeekType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absolute => write!(f, "Absolute"),
            Self::RelativeForwards => write!(f, "Forwards"),
            Self::RelativeBackwards => write!(f, "Backwards"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum RepeatMode {
    /// No repeat: after the queue is finished the player stops
    None,
    /// Repeat Once: after going through the queue once, the player goes back to `RepeatMode::None` and continues
    Once,
    /// Repeat Continuously: after going through the queue, the player goes back to the beginning and continues
    Continuous,
}

impl Display for RepeatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Once => write!(f, "Once"),
            Self::Continuous => write!(f, "Continuous"),
        }
    }
}

impl RepeatMode {
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    #[must_use]
    pub const fn is_once(&self) -> bool {
        matches!(self, Self::Once)
    }

    #[must_use]
    pub const fn is_continuous(&self) -> bool {
        matches!(self, Self::Continuous)
    }
}

#[nutype(
    sanitize(with = | n | if n.is_finite() { n.clamp(0.0, 100.0) } else { 0.0 }),
    derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)
)]
pub struct Percent(f32);

impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}%", self.into_inner())
    }
}

/// Information about the runtime of the current song
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct StateRuntime {
    pub seek_position: f64,
    pub seek_percent: Percent,
    pub duration: Duration,
}

impl Display for StateRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateRuntime {{ seek_position: {:.2}s, seek_percent: {}, duration: {:.1}s }}",
            self.seek_position,
            self.seek_percent,
            self.duration.as_secs_f32()
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateAudio {
    pub queue: Box<[Song]>,
    pub queue_position: Option<usize>,
    pub current_song: Option<Song>,
    pub repeat_mode: RepeatMode,
    pub runtime: Option<StateRuntime>,
    pub paused: bool,
    pub muted: bool,
    pub volume: f32,
}

impl Display for StateAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateAudio {{ queue: {:?}, queue_position: {}, current_song: {}, repeat_mode: {}, runtime: {}, paused: {}, muted: {}, volume: {:.0}% }}",
            self.queue
                .iter()
                .map(|song| song.title.to_string())
                .collect::<Vec<_>>(),
            self.queue_position.map_or_else(|| "None".to_string(), |pos| pos.to_string()),
            self.current_song.as_ref().map_or_else(|| "None".to_string(),|song| format!("\"{}\"",song.title)),
            self.repeat_mode,
            self.runtime.as_ref().map_or_else(|| "None".to_string(),std::string::ToString::to_string),
            self.paused,
            self.muted,
            self.volume * 100.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use one_or_many::OneOrMany;
    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

    #[rstest]
    #[case::none(RepeatMode::None, [true, false, false])]
    #[case::once(RepeatMode::Once, [false, true, false])]
    #[case::continuous(RepeatMode::Continuous, [false, false, true])]
    fn test_repeat_mode(#[case] mode: RepeatMode, #[case] expected: [bool; 3]) {
        assert_eq!(mode.is_none(), expected[0]);
        assert_eq!(mode.is_once(), expected[1]);
        assert_eq!(mode.is_continuous(), expected[2]);
    }

    #[rstest]
    #[case::seek_type(SeekType::Absolute, "Absolute")]
    #[case::seek_type(SeekType::RelativeForwards, "Forwards")]
    #[case::seek_type(SeekType::RelativeBackwards, "Backwards")]
    #[case::repeat_mode(RepeatMode::None, "None")]
    #[case::repeat_mode(RepeatMode::Once, "Once")]
    #[case::repeat_mode(RepeatMode::Continuous, "Continuous")]
    #[case::percent(Percent::new(50.0), "50.00%")]
    #[case::state_runtimme(
        StateRuntime {
            seek_position: 3.0,
            seek_percent: Percent::new(50.0),
            duration: Duration::from_secs(6),
        },
        "StateRuntime { seek_position: 3.00s, seek_percent: 50.00%, duration: 6.0s }"
    )]
    #[case::state_audio_empty(
        StateAudio {
            queue: Box::new([]),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::None,
            runtime: None,
            paused: false,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [], queue_position: None, current_song: None, repeat_mode: None, runtime: None, paused: false, muted: false, volume: 100% }"
    )]
    #[case::state_audio(
        StateAudio {
            queue: Box::new([
                Song {
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
                Song {
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
                seek_position: 20.0,
                seek_percent: Percent::new(20.0),
                duration: Duration::from_secs(100),
            }),
            paused: false,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [\"Song 1\"], queue_position: 1, current_song: \"Song 1\", repeat_mode: None, runtime: StateRuntime { seek_position: 20.00s, seek_percent: 20.00%, duration: 100.0s }, paused: false, muted: false, volume: 100% }"
    )]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
