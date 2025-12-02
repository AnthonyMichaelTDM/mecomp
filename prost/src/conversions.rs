//! Conversions between prost-generated types and core types.

use std::time::Duration;

use mecomp_storage::db::schemas::dynamic::query::Compile;

#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn convert_std_duration(duration: Duration) -> prost_types::Duration {
    prost_types::Duration {
        seconds: duration.as_secs().clamp(0, i64::MAX as u64) as i64,
        nanos: duration.subsec_nanos().clamp(0, i32::MAX as u32) as i32,
    }
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
            runtime: value.runtime.normalized().try_into().unwrap_or_default(),
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            extension: value.extension,
            path: value.path.into(),
        }
    }
}

impl From<mecomp_storage::db::schemas::song::Song> for crate::Song {
    fn from(value: mecomp_storage::db::schemas::song::Song) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            album_artists: value.album_artist.into(),
            album: value.album,
            genres: value.genre.into(),
            runtime: convert_std_duration(value.runtime),
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            extension: value.extension,
            path: format!("{}", value.path.display()),
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
            runtime: value.runtime.normalized().try_into().unwrap_or_default(),
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            path: value.path.into(),
        }
    }
}

impl From<mecomp_storage::db::schemas::song::SongBrief> for crate::SongBrief {
    fn from(value: mecomp_storage::db::schemas::song::SongBrief) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            album_artists: value.album_artist.into(),
            album: value.album,
            genres: value.genre.into(),
            runtime: convert_std_duration(value.runtime),
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            path: format!("{}", value.path.display()),
        }
    }
}

impl From<mecomp_storage::db::schemas::song::Song> for crate::SongBrief {
    fn from(value: mecomp_storage::db::schemas::song::Song) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            album_artists: value.album_artist.into(),
            album: value.album,
            genres: value.genre.into(),
            runtime: convert_std_duration(value.runtime),
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            path: format!("{}", value.path.display()),
        }
    }
}

impl From<crate::Song> for crate::SongBrief {
    fn from(value: crate::Song) -> Self {
        Self {
            id: value.id,
            title: value.title,
            artists: value.artists,
            album_artists: value.album_artists,
            album: value.album,
            genres: value.genres,
            runtime: value.runtime,
            track: value.track,
            disc: value.disc,
            release_year: value.release_year,
            path: value.path,
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
            seek_position: value
                .seek_position
                .normalized()
                .try_into()
                .unwrap_or_default(),
            seek_percent: mecomp_core::state::Percent::new(value.seek_percent),
            duration: value.duration.normalized().try_into().unwrap_or_default(),
        }
    }
}

impl From<mecomp_core::state::StateRuntime> for crate::StateRuntime {
    fn from(value: mecomp_core::state::StateRuntime) -> Self {
        Self {
            seek_position: convert_std_duration(value.seek_position),
            seek_percent: value.seek_percent.into_inner(),
            duration: convert_std_duration(value.duration),
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

impl From<mecomp_core::state::StateAudio> for crate::StateAudio {
    fn from(value: mecomp_core::state::StateAudio) -> Self {
        Self {
            queue: value.queue.into_iter().map(Into::into).collect(),
            queue_position: value.queue_position.map(|v| v as u64),
            current_song: value.current_song.map(Into::into),
            repeat_mode: crate::RepeatMode::from(value.repeat_mode) as i32,
            runtime: value.runtime.map(Into::into),
            status: crate::PlaybackStatus::from(value.status) as i32,
            muted: value.muted,
            volume: value.volume,
        }
    }
}

impl From<mecomp_storage::db::schemas::RecordId> for crate::RecordId {
    fn from(value: mecomp_storage::db::schemas::RecordId) -> Self {
        Self {
            id: value.id.to_string(),
            tb: value.tb,
        }
    }
}

impl From<crate::RecordId> for mecomp_storage::db::schemas::RecordId {
    fn from(value: crate::RecordId) -> Self {
        Self {
            id: mecomp_storage::db::schemas::Id::String(value.id),
            tb: value.tb,
        }
    }
}

impl From<crate::SeekType> for mecomp_core::state::SeekType {
    fn from(value: crate::SeekType) -> Self {
        match value {
            crate::SeekType::Unspecified | crate::SeekType::Absolute => Self::Absolute,
            crate::SeekType::RelativeForwards => Self::RelativeForwards,
            crate::SeekType::RelativeBackwards => Self::RelativeBackwards,
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
        Self::new(value.into())
    }
}

impl From<crate::RecordId> for crate::Ulid {
    fn from(value: crate::RecordId) -> Self {
        Self { id: value.id }
    }
}

impl From<mecomp_storage::db::schemas::artist::Artist> for crate::Artist {
    fn from(value: mecomp_storage::db::schemas::artist::Artist) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
            runtime: convert_std_duration(value.runtime),
            album_count: value.album_count,
            song_count: value.song_count,
        }
    }
}

impl From<mecomp_storage::db::schemas::artist::Artist> for crate::ArtistBrief {
    fn from(value: mecomp_storage::db::schemas::artist::Artist) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
        }
    }
}

impl From<mecomp_storage::db::schemas::artist::ArtistBrief> for crate::ArtistBrief {
    fn from(value: mecomp_storage::db::schemas::artist::ArtistBrief) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
        }
    }
}

impl From<crate::Artist> for crate::ArtistBrief {
    fn from(value: crate::Artist) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<mecomp_storage::db::schemas::album::Album> for crate::Album {
    fn from(value: mecomp_storage::db::schemas::album::Album) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            release: value.release,
            runtime: convert_std_duration(value.runtime),
            song_count: value.song_count,
            discs: value.discs,
            genres: value.genre.into(),
        }
    }
}

impl From<mecomp_storage::db::schemas::album::AlbumBrief> for crate::AlbumBrief {
    fn from(value: mecomp_storage::db::schemas::album::AlbumBrief) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            release: value.release,
            discs: value.discs,
            genres: value.genre.into(),
        }
    }
}

impl From<mecomp_storage::db::schemas::album::Album> for crate::AlbumBrief {
    fn from(value: mecomp_storage::db::schemas::album::Album) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            title: value.title,
            artists: value.artist.into(),
            release: value.release,
            discs: value.discs,
            genres: value.genre.into(),
        }
    }
}

impl From<crate::Album> for crate::AlbumBrief {
    fn from(value: crate::Album) -> Self {
        Self {
            id: value.id,
            title: value.title,
            artists: value.artists,
            release: value.release,
            discs: value.discs,
            genres: value.genres,
        }
    }
}

impl From<mecomp_storage::db::schemas::playlist::Playlist> for crate::Playlist {
    fn from(value: mecomp_storage::db::schemas::playlist::Playlist) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
            runtime: convert_std_duration(value.runtime),
            song_count: value.song_count,
        }
    }
}
impl From<mecomp_storage::db::schemas::playlist::PlaylistBrief> for crate::PlaylistBrief {
    fn from(value: mecomp_storage::db::schemas::playlist::PlaylistBrief) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
        }
    }
}
impl From<mecomp_storage::db::schemas::playlist::Playlist> for crate::PlaylistBrief {
    fn from(value: mecomp_storage::db::schemas::playlist::Playlist) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
        }
    }
}

impl From<crate::Playlist> for crate::PlaylistBrief {
    fn from(value: crate::Playlist) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<mecomp_storage::db::schemas::collection::Collection> for crate::Collection {
    fn from(value: mecomp_storage::db::schemas::collection::Collection) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
            runtime: convert_std_duration(value.runtime),
            song_count: value.song_count,
        }
    }
}

impl From<mecomp_storage::db::schemas::collection::CollectionBrief> for crate::CollectionBrief {
    fn from(value: mecomp_storage::db::schemas::collection::CollectionBrief) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
        }
    }
}

impl From<crate::Collection> for crate::CollectionBrief {
    fn from(value: crate::Collection) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<mecomp_storage::db::schemas::dynamic::DynamicPlaylist> for crate::DynamicPlaylist {
    fn from(value: mecomp_storage::db::schemas::dynamic::DynamicPlaylist) -> Self {
        Self {
            id: crate::RecordId::new(value.id.table(), value.id.key()),
            name: value.name,
            query: value.query.compile_for_storage(),
        }
    }
}
