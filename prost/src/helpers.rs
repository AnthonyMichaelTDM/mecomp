//! Helper functions to make creating and manipulating prost types easier.

use core::fmt;
use std::str::FromStr;

impl crate::SearchResult {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.songs.len() + self.albums.len() + self.artists.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl crate::SearchRequest {
    #[must_use]
    pub fn new(query: impl Into<String>, limit: u64) -> Self {
        Self {
            query: query.into(),
            limit,
        }
    }
}

impl crate::LibraryAnalyzeRequest {
    #[must_use]
    pub const fn new(overwrite: bool) -> Self {
        Self { overwrite }
    }
}

impl crate::RecordId {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(tb: impl ToString, id: impl ToString) -> Self {
        Self {
            tb: tb.to_string(),
            id: id.to_string(),
        }
    }

    #[must_use]
    pub fn ulid(&self) -> crate::Ulid {
        crate::Ulid {
            id: self.id.clone(),
        }
    }
}

impl<S, U> From<(S, U)> for crate::RecordId
where
    S: ToString,
    U: Into<crate::Ulid>,
{
    fn from((tb, id): (S, U)) -> Self {
        Self::new(tb, id.into())
    }
}

impl FromStr for crate::RecordId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        mecomp_storage::db::schemas::RecordId::from_str(s).map(Into::into)
    }
}

impl fmt::Display for crate::RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.tb, self.id)
    }
}

impl crate::Ulid {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(id: impl ToString) -> Self {
        Self { id: id.to_string() }
    }
}

impl fmt::Display for crate::Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl crate::RecordIdList {
    #[must_use]
    pub const fn new(ids: Vec<crate::RecordId>) -> Self {
        Self { ids }
    }
}

impl crate::PlaybackSkipRequest {
    #[must_use]
    pub const fn new(amount: u64) -> Self {
        Self { amount }
    }
}

impl crate::PlaybackRepeatRequest {
    #[must_use]
    pub fn new(mode: impl Into<crate::RepeatMode>) -> Self {
        Self {
            mode: mode.into() as i32,
        }
    }
}

impl crate::PlaybackSeekRequest {
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn new(mode: impl Into<crate::SeekType>, duration: std::time::Duration) -> Self {
        Self {
            seek: mode.into() as i32,
            duration: prost_types::Duration {
                seconds: duration.as_secs() as i64,
                nanos: duration.subsec_nanos() as i32,
            }
            .normalized(),
        }
    }
}

impl crate::PlaybackVolumeSetRequest {
    #[must_use]
    pub const fn new(volume: f32) -> Self {
        Self { volume }
    }
}

impl crate::PlaybackVolumeAdjustRequest {
    #[must_use]
    pub const fn new(amount: f32) -> Self {
        Self { amount }
    }
}

impl crate::PlaylistAddRequest {
    #[must_use]
    pub fn new(playlist_id: impl Into<crate::Ulid>, record_id: crate::RecordId) -> Self {
        Self {
            playlist_id: playlist_id.into(),
            record_id,
        }
    }
}

impl crate::PlaylistAddListRequest {
    #[must_use]
    pub fn new(playlist_id: impl Into<crate::Ulid>, record_ids: Vec<crate::RecordId>) -> Self {
        Self {
            playlist_id: playlist_id.into(),
            record_ids,
        }
    }
}

impl crate::PlaylistRenameRequest {
    #[must_use]
    pub fn new(playlist_id: impl Into<crate::Ulid>, new_name: impl Into<String>) -> Self {
        Self {
            playlist_id: playlist_id.into(),
            name: new_name.into(),
        }
    }
}

impl crate::PlaylistRemoveSongsRequest {
    #[must_use]
    pub fn new(playlist_id: impl Into<crate::Ulid>, song_ids: Vec<impl Into<crate::Ulid>>) -> Self {
        Self {
            playlist_id: playlist_id.into(),
            song_ids: song_ids.into_iter().map(Into::into).collect(),
        }
    }
}

impl crate::QueueRemoveRangeRequest {
    #[must_use]
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }
}

impl crate::QueueSetIndexRequest {
    #[must_use]
    pub const fn new(index: u64) -> Self {
        Self { index }
    }
}

impl crate::DynamicPlaylistCreateRequest {
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn new(name: impl ToString, query: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            query: query.to_string(),
        }
    }
}

impl crate::DynamicPlaylistUpdateRequest {
    #[must_use]
    pub fn new(id: impl Into<crate::Ulid>, changes: crate::DynamicPlaylistChangeSet) -> Self {
        Self {
            id: id.into(),
            changes,
        }
    }
}

impl crate::DynamicPlaylistChangeSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            new_name: None,
            new_query: None,
        }
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.new_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.new_query = Some(query.into());
        self
    }
}

impl crate::RadioSimilarRequest {
    #[must_use]
    pub const fn new(record_ids: Vec<crate::RecordId>, limit: u32) -> Self {
        Self { record_ids, limit }
    }
}

impl crate::CollectionFreezeRequest {
    pub fn new(id: impl ToString, name: impl Into<String>) -> Self {
        Self {
            id: crate::Ulid::new(id),
            name: name.into(),
        }
    }
}

impl crate::RegisterListenerRequest {
    #[must_use]
    pub fn new(addr: std::net::SocketAddr) -> Self {
        Self {
            host: addr.ip().to_string(),
            port: u32::from(addr.port()),
        }
    }
}

impl crate::Path {
    #[must_use]
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: format!("{}", path.as_ref().display()),
        }
    }
}

impl crate::PlaylistName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl crate::PlaylistExportRequest {
    #[must_use]
    pub fn new(playlist_id: impl Into<crate::Ulid>, path: impl AsRef<std::path::Path>) -> Self {
        Self {
            playlist_id: playlist_id.into(),
            path: format!("{}", path.as_ref().display()),
        }
    }
}

impl crate::PlaylistImportRequest {
    #[must_use]
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: format!("{}", path.as_ref().display()),
            name: None,
        }
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn with_name(path: impl AsRef<std::path::Path>, name: impl ToString) -> Self {
        Self {
            path: format!("{}", path.as_ref().display()),
            name: Some(name.to_string()),
        }
    }
}
