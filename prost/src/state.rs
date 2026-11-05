//! helpers for types that model the music player state that were formally defined in `mecomp-core`

use std::fmt::Display;

use crate::{PlaybackStatus, RepeatMode, SeekType, StateAudio, StateRuntime};

impl Display for RepeatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unspecified | Self::None => write!(f, "None"),
            Self::One => write!(f, "One"),
            Self::All => write!(f, "All"),
        }
    }
}

impl RepeatMode {
    #[must_use]
    #[inline]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::Unspecified | Self::None)
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

    #[must_use]
    #[inline]
    pub const fn next(&self) -> Self {
        match self {
            Self::Unspecified | Self::None => Self::All,
            Self::All => Self::One,
            Self::One => Self::None,
        }
    }
}

impl Display for PlaybackStatus {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paused => write!(f, "Paused"),
            Self::Playing => write!(f, "Playing"),
            Self::Unspecified | Self::Stopped => write!(f, "Stopped"),
        }
    }
}

impl Display for SeekType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unspecified | Self::Absolute => write!(f, "Absolute"),
            Self::RelativeForwards => write!(f, "Forwards"),
            Self::RelativeBackwards => write!(f, "Backwards"),
        }
    }
}

impl Display for StateRuntime {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateRuntime {{ seek_position: {}, duration: {} }}",
            self.seek_position, self.duration
        )
    }
}

impl StateAudio {
    #[must_use]
    #[inline]
    pub fn paused(&self) -> bool {
        !matches!(self.status(), PlaybackStatus::Playing)
    }
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
            self.repeat_mode(),
            self.runtime
                .as_ref()
                .map_or_else(|| "None".to_string(), ToString::to_string),
            self.status(),
            self.muted,
            self.volume * 100.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{Duration, RecordId, SongBrief};

    use super::*;

    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

    fn generate_song_id() -> RecordId {
        let id = mecomp_storage::db::schemas::song::Song::generate_id();
        RecordId::from(id)
    }

    #[rstest]
    #[case::unspecified(RepeatMode::Unspecified, [true, false, false])]
    #[case::none(RepeatMode::None, [true, false, false])]
    #[case::one(RepeatMode::One, [false, true, false])]
    #[case::all(RepeatMode::All, [false, false, true])]
    fn test_repeat_mode(#[case] mode: RepeatMode, #[case] expected: [bool; 3]) {
        assert_eq!(mode.is_none(), expected[0]);
        assert_eq!(mode.is_one(), expected[1]);
        assert_eq!(mode.is_all(), expected[2]);
    }

    #[rstest]
    #[case::unspecified(RepeatMode::Unspecified, RepeatMode::All)]
    #[case::none(RepeatMode::None, RepeatMode::All)]
    #[case::all(RepeatMode::All, RepeatMode::One)]
    #[case::one(RepeatMode::One, RepeatMode::None)]
    fn test_repeat_mode_next(#[case] mode: RepeatMode, #[case] expected: RepeatMode) {
        assert_eq!(mode.next(), expected);
    }

    #[rstest]
    #[case::playback_status_paused(PlaybackStatus::Paused, "Paused")]
    #[case::playback_status_playing(PlaybackStatus::Playing, "Playing")]
    #[case::playback_status_stopped(PlaybackStatus::Stopped, "Stopped")]
    #[case::playback_status_unspecified(PlaybackStatus::Unspecified, "Stopped")]
    #[case::playback_status_default(PlaybackStatus::default(), "Stopped")]
    #[case::seek_type(SeekType::Absolute, "Absolute")]
    #[case::seek_type(SeekType::RelativeForwards, "Forwards")]
    #[case::seek_type(SeekType::RelativeBackwards, "Backwards")]
    #[case::seek_type_unspecified(SeekType::Unspecified, "Absolute")]
    #[case::seek_type_default(SeekType::default(), "Absolute")]
    #[case::repeat_mode(RepeatMode::None, "None")]
    #[case::repeat_mode(RepeatMode::One, "One")]
    #[case::repeat_mode(RepeatMode::All, "All")]
    #[case::repeat_mode_unspecified(RepeatMode::Unspecified, "None")]
    #[case::repeat_mode_default(RepeatMode::default(), "None")]
    #[case::state_runtimme(
        StateRuntime {
            seek_position: Duration::from_secs(3),
            duration: Duration::from_secs(6),
        },
        "StateRuntime { seek_position: 0:03.0, duration: 0:06.0 }"
    )]
    #[case::state_audio_empty(
        StateAudio {
            queue: Vec::new(),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::None as i32,
            runtime: None,
            status: PlaybackStatus::Paused as i32,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [], queue_position: None, current_song: None, repeat_mode: None, runtime: None, status: Paused, muted: false, volume: 100% }"
    )]
    #[case::state_audio_empty(
        StateAudio {
            queue: Vec::new(),
            queue_position: None,
            current_song: None,
            repeat_mode: RepeatMode::None as i32,
            runtime: None,
            status: PlaybackStatus::Paused as i32,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [], queue_position: None, current_song: None, repeat_mode: None, runtime: None, status: Paused, muted: false, volume: 100% }"
    )]
    #[case::state_audio(
        StateAudio {
            queue: vec![
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
            ],
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
            repeat_mode: RepeatMode::None as i32,
            runtime: Some(StateRuntime {
                seek_position: Duration::from_secs(20),
                duration: Duration::from_secs(100),
            }),
            status: PlaybackStatus::Playing as i32,
            muted: false,
            volume: 1.0,
        },
        "StateAudio { queue: [\"Song 1\"], queue_position: 1, current_song: \"Song 1\", repeat_mode: None, runtime: StateRuntime { seek_position: 0:20.0, duration: 1:40.0 }, status: Playing, muted: false, volume: 100% }"
    )]
    // #[case::percent(Percent::new(50.0), "50.00%")]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
