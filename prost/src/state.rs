//! helpers for types that model the music player state that were formally defined in `mecomp-core`

use std::fmt::Display;

use crate::{PlaybackStatus, RepeatMode, SeekType};

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

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

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
    // #[case::percent(Percent::new(50.0), "50.00%")]
    fn test_display_impls<T: Display>(#[case] input: T, #[case] expected: &str) {
        assert_str_eq!(input.to_string(), expected);
    }
}
