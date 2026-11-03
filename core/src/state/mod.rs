#![allow(clippy::module_name_repetitions)]
use std::fmt::Display;
use std::time::Duration;

use mecomp_prost::{PlaybackStatus, RepeatMode, convert_std_duration};
use mecomp_storage::db::schemas::song::SongBrief;
use serde::{Deserialize, Serialize};

use crate::format_duration;

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

impl From<mecomp_prost::StateRuntime> for StateRuntime {
    #[inline]
    fn from(value: mecomp_prost::StateRuntime) -> Self {
        Self {
            seek_position: value
                .seek_position
                .normalized()
                .try_into()
                .unwrap_or_default(),
            seek_percent: Percent::new(value.seek_percent),
            duration: value.duration.normalized().try_into().unwrap_or_default(),
        }
    }
}

impl From<StateRuntime> for mecomp_prost::StateRuntime {
    #[inline]
    fn from(value: StateRuntime) -> Self {
        Self {
            seek_position: convert_std_duration(value.seek_position),
            seek_percent: value.seek_percent.into_inner(),
            duration: convert_std_duration(value.duration),
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
    pub status: PlaybackStatus,
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
                .map(|song| &song.title)
                .collect::<Vec<_>>(),
            self.queue_position
                .map_or_else(|| "None".to_string(), |pos| pos.to_string()),
            self.current_song
                .as_ref()
                .map_or_else(|| "None".to_string(), |song| format!("\"{}\"", song.title)),
            self.repeat_mode,
            self.runtime
                .as_ref()
                .map_or_else(|| "None".to_string(), ToString::to_string),
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
            repeat_mode: RepeatMode::None,
            runtime: None,
            status: PlaybackStatus::Stopped,
            muted: false,
            volume: 1.0,
        }
    }
}

impl From<mecomp_prost::StateAudio> for StateAudio {
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    fn from(value: mecomp_prost::StateAudio) -> Self {
        Self {
            queue: value.queue.into_iter().map(Into::into).collect(),
            queue_position: value.queue_position.map(|v| v as usize),
            current_song: value.current_song.map(Into::into),
            repeat_mode: RepeatMode::try_from(value.repeat_mode).unwrap_or_default(),
            runtime: value.runtime.map(Into::into),
            status: PlaybackStatus::try_from(value.status).unwrap_or_default(),
            muted: value.muted,
            volume: value.volume,
        }
    }
}

impl From<StateAudio> for mecomp_prost::StateAudio {
    #[inline]
    fn from(value: StateAudio) -> Self {
        Self {
            queue: value.queue.into_iter().map(Into::into).collect(),
            queue_position: value.queue_position.map(|v| v as u64),
            current_song: value.current_song.map(Into::into),
            repeat_mode: value.repeat_mode as i32,
            runtime: value.runtime.map(Into::into),
            status: value.status as i32,
            muted: value.muted,
            volume: value.volume,
        }
    }
}

impl StateAudio {
    #[must_use]
    #[inline]
    pub const fn paused(&self) -> bool {
        !matches!(self.status, PlaybackStatus::Playing)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use mecomp_storage::db::schemas::song::Song;
    use one_or_many::OneOrMany;
    use pretty_assertions::assert_eq;
    use pretty_assertions::assert_str_eq;
    use rstest::rstest;

    #[test]
    fn test_state_audio_default() {
        let state = StateAudio::default();
        assert_eq!(state.queue.as_ref(), &[]);
        assert_eq!(state.queue_position, None);
        assert_eq!(state.current_song, None);
        assert_eq!(state.repeat_mode, RepeatMode::None);
        assert_eq!(state.runtime, None);
        assert_eq!(state.status, PlaybackStatus::Stopped);
        assert_eq!(state.muted, false);
        assert!(
            f32::EPSILON > (state.volume - 1.0).abs(),
            "{} != 1.0",
            state.volume
        );
    }

    #[rstest]
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
            status: PlaybackStatus::Paused,
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
            status: PlaybackStatus::Paused,
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
                    path: "foo/bar.mp3".into(),
                }
            ),
            repeat_mode: RepeatMode::None,
            runtime: Some(StateRuntime {
                seek_position: Duration::from_secs(20),
                seek_percent: Percent::new(20.0),
                duration: Duration::from_secs(100),
            }),
            status: PlaybackStatus::Playing,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [\"Song 1\"], queue_position: 1, current_song: \"Song 1\", repeat_mode: None, runtime: StateRuntime { seek_position: 00:00:20.00, seek_percent: 20.00%, duration: 00:01:40.00 }, status: Playing, muted: false, volume: 100% }"
    )]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
