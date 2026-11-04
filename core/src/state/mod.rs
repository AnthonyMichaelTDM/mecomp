#![allow(clippy::module_name_repetitions)]
use std::fmt::Display;

use mecomp_prost::{PlaybackStatus, RepeatMode, SongBrief, StateRuntime};
use serde::{Deserialize, Serialize};

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
            queue: value.queue.into_boxed_slice(),
            queue_position: value.queue_position.map(|v| v as usize),
            current_song: value.current_song,
            repeat_mode: RepeatMode::try_from(value.repeat_mode).unwrap_or_default(),
            runtime: value.runtime,
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
            queue: value.queue.to_vec(),
            queue_position: value.queue_position.map(|v| v as u64),
            current_song: value.current_song,
            repeat_mode: value.repeat_mode as i32,
            runtime: value.runtime,
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
    use super::*;
    use mecomp_prost::Duration;
    use mecomp_storage::db::schemas::song::Song;
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

    fn generate_song_id() -> mecomp_prost::RecordId {
        let id = Song::generate_id();
        mecomp_prost::RecordId::from((id.table(), id.key().to_string()))
    }

    #[rstest]
    #[case::state_runtimme(
        StateRuntime {
            seek_position: Duration::from_secs(3),
            duration: Duration::from_secs(6),
        },
        "StateRuntime { seek_position: 0:03.0, duration: 0:06.0 }"
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
                    id: generate_song_id(),
                    title: "Song 1".into(),
                    artists: Vec::new(),
                    album_artists: Vec::new(),
                    album: "album".into(),
                    genres: Vec::new(),
                    runtime: Duration::from_secs(100).into(),
                    track: None,
                    disc: None,
                    release_year: None,
                    path: "foo/bar.mp3".into(),
                }
            ]),
            queue_position: Some(1),
            current_song: Some(
                SongBrief {
                    id: generate_song_id(),
                    title: "Song 1".into(),
                    artists: Vec::new(),
                    album_artists: Vec::new(),
                    album: "album".into(),
                    genres: Vec::new(),
                    runtime: Duration::from_secs(100).into(),
                    track: None,
                    disc: None,
                    release_year: None,
                    path: "foo/bar.mp3".into(),
                }
            ),
            repeat_mode: RepeatMode::None,
            runtime: Some(StateRuntime {
                seek_position: Duration::from_secs(20),
                duration: Duration::from_secs(100),
            }),
            status: PlaybackStatus::Playing,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [\"Song 1\"], queue_position: 1, current_song: \"Song 1\", repeat_mode: None, runtime: StateRuntime { seek_position: 0:20.0, seek_percent: 20.00%, duration: 1:40.0 }, status: Playing, muted: false, volume: 100% }"
    )]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
