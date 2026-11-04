//! Conversions between prost-generated types and core types.

use mecomp_storage::db::schemas::dynamic::query::Compile;

impl From<crate::Song> for mecomp_storage::db::schemas::song::Song {
    fn from(value: crate::Song) -> Self {
        Self {
            id: mecomp_storage::db::schemas::RecordId::from(value.id).into(),
            title: value.title,
            artist: value.artists.into(),
            album_artist: value.album_artists.into(),
            album: value.album,
            genre: value.genres.into(),
            runtime: *value.runtime.inner(),
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
            runtime: value.runtime.into(),
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
            runtime: *value.runtime.inner(),
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
            runtime: value.runtime.into(),
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
            runtime: value.runtime.into(),
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
            runtime: value.runtime.into(),
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
            runtime: value.runtime.into(),
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
            runtime: value.runtime.into(),
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
            runtime: value.runtime.into(),
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
