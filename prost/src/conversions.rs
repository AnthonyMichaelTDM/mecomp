//! Conversions between prost-generated types and core types.

use std::time::Duration;

#[allow(clippy::cast_sign_loss)]
pub const fn convert_duration(duration: prost_types::Duration) -> Duration {
    Duration::new(duration.seconds as u64, duration.nanos as u32)
}

impl From<crate::Song> for mecomp_storage::db::schemas::song::Song {
    fn from(value: crate::Song) -> Self {
        Self {
            id: mecomp_storage::db::schemas::RecordId::from(value.id).into(),
            title: value.title,
            artist: value.artists.into(),
            album_artist: value.album_artists.into(),
            album: value.album,
            genre: value.genres.into(),
            runtime: convert_duration(value.runtime),
            track: value.track,
            disc: value.disc,
            release: value.release,
            extension: value.extension,
            path: value.path.into(),
        }
    }
}

impl From<crate::SongBrief> for mecomp_storage::db::schemas::song::SongBrief {
    fn from(value: crate::SongBrief) -> Self {
        Self {
            id: mecomp_storage::db::schemas::RecordId::from(value.id).into(),
            title: value.title,
            artist: value.artists.into(),
            album_artist: value.album_artists.into(),
            album: value.album,
            genre: value.genres.into(),
            runtime: convert_duration(value.runtime),
            track: value.track,
            disc: value.disc,
            release_year: value.release,
            path: value.path.into(),
        }
    }
}

impl From<crate::RepeatMode> for mecomp_core::state::RepeatMode {
    fn from(value: crate::RepeatMode) -> Self {
        match value {
            crate::RepeatMode::None | crate::RepeatMode::Unspecified => Self::None,
            crate::RepeatMode::One => Self::One,
            crate::RepeatMode::All => Self::All,
        }
    }
}

impl From<mecomp_core::state::RepeatMode> for crate::RepeatMode {
    fn from(val: mecomp_core::state::RepeatMode) -> Self {
        match val {
            mecomp_core::state::RepeatMode::None => Self::None,
            mecomp_core::state::RepeatMode::One => Self::One,
            mecomp_core::state::RepeatMode::All => Self::All,
        }
    }
}

impl From<crate::PlaybackStatus> for mecomp_core::state::Status {
    fn from(value: crate::PlaybackStatus) -> Self {
        match value {
            crate::PlaybackStatus::Unspecified | crate::PlaybackStatus::Stopped => Self::Stopped,
            crate::PlaybackStatus::Playing => Self::Playing,
            crate::PlaybackStatus::Paused => Self::Paused,
        }
    }
}

impl From<mecomp_core::state::Status> for crate::PlaybackStatus {
    fn from(val: mecomp_core::state::Status) -> Self {
        match val {
            mecomp_core::state::Status::Stopped => Self::Stopped,
            mecomp_core::state::Status::Playing => Self::Playing,
            mecomp_core::state::Status::Paused => Self::Paused,
        }
    }
}

impl From<crate::StateRuntime> for mecomp_core::state::StateRuntime {
    fn from(value: crate::StateRuntime) -> Self {
        Self {
            seek_position: convert_duration(value.seek_position),
            seek_percent: mecomp_core::state::Percent::new(value.seek_percent),
            duration: convert_duration(value.duration),
        }
    }
}

impl From<crate::StateAudio> for mecomp_core::state::StateAudio {
    #[allow(clippy::cast_possible_truncation)]
    fn from(value: crate::StateAudio) -> Self {
        Self {
            queue: value.queue.into_iter().map(Into::into).collect(),
            queue_position: value.queue_position.map(|v| v as usize),
            current_song: value.current_song.map(Into::into),
            repeat_mode: crate::RepeatMode::try_from(value.repeat_mode)
                .map(Into::into)
                .unwrap_or_default(),
            runtime: value.runtime.map(Into::into),
            status: crate::PlaybackStatus::try_from(value.status)
                .map(Into::into)
                .unwrap_or_default(),
            muted: value.muted,
            volume: value.volume,
        }
    }
}

impl From<mecomp_storage::db::schemas::RecordId> for crate::RecordId {
    fn from(value: mecomp_storage::db::schemas::RecordId) -> Self {
        Self {
            id: crate::Ulid::new(value.id.to_string()),
            tb: value.tb,
        }
    }
}

impl From<crate::RecordId> for mecomp_storage::db::schemas::RecordId {
    fn from(value: crate::RecordId) -> Self {
        Self {
            id: mecomp_storage::db::schemas::Id::String(value.id.ulid),
            tb: value.tb,
        }
    }
}

impl From<mecomp_core::state::SeekType> for crate::SeekType {
    fn from(val: mecomp_core::state::SeekType) -> Self {
        match val {
            mecomp_core::state::SeekType::Absolute => Self::Absolute,
            mecomp_core::state::SeekType::RelativeForwards => Self::RelativeForwards,
            mecomp_core::state::SeekType::RelativeBackwards => Self::RelativeBackwards,
        }
    }
}

impl<T> From<T> for crate::Ulid
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl From<crate::RecordId> for crate::Ulid {
    fn from(value: crate::RecordId) -> Self {
        value.id
    }
}
