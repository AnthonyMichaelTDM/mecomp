use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::RecordId;

pub type PlaylistId = RecordId;

pub const TABLE_NAME: &str = "playlist";

/// This struct holds all the metadata about a particular [`Playlist`].
/// A [`Playlist`] is a collection of [`super::song::Song`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("playlist"))]
pub struct Playlist {
    /// the unique identifier for this [`Playlist`].
    #[cfg_attr(feature = "db", field("record"))]
    pub id: PlaylistId,

    /// The [`Playlist`]'s name.
    #[cfg_attr(feature = "db", field(dt = "string", index(unique)))]
    pub name: String,

    /// Total runtime.
    #[cfg_attr(feature = "db", field(dt = "duration"))]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,

    /// the number of songs this playlist has.
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub song_count: usize,
}

impl Playlist {
    #[must_use]
    #[inline]
    pub fn generate_id() -> PlaylistId {
        RecordId::from_table_key(TABLE_NAME, Id::ulid())
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PlaylistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(
        feature = "db",
        serde(serialize_with = "super::serialize_duration_option_as_sql_duration")
    )]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub song_count: Option<usize>,
}

impl PlaylistChangeSet {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    #[inline]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    #[inline]
    pub const fn runtime(mut self, runtime: Duration) -> Self {
        self.runtime = Some(runtime);
        self
    }

    #[must_use]
    #[inline]
    pub const fn song_count(mut self, song_count: usize) -> Self {
        self.song_count = Some(song_count);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistBrief {
    pub id: PlaylistId,
    pub name: String,
    pub runtime: std::time::Duration,
    pub songs: usize,
}

impl From<Playlist> for PlaylistBrief {
    #[inline]
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            runtime: playlist.runtime,
            songs: playlist.song_count,
        }
    }
}

impl From<&Playlist> for PlaylistBrief {
    #[inline]
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            runtime: playlist.runtime,
            songs: playlist.song_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn playlist() -> Playlist {
        Playlist {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "playlist".into(),
            runtime: Duration::from_secs(3600),
            song_count: 100,
        }
    }

    #[fixture]
    fn playlist_brief() -> PlaylistBrief {
        PlaylistBrief {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "playlist".into(),
            runtime: Duration::from_secs(3600),
            songs: 100,
        }
    }

    #[rstest]
    #[case(playlist(), playlist_brief())]
    #[case(&playlist(), playlist_brief())]
    fn test_playlist_brief_from_playlist<T: Into<PlaylistBrief>>(
        #[case] playlist: T,
        #[case] brief: PlaylistBrief,
    ) {
        let actual: PlaylistBrief = playlist.into();
        assert_eq!(actual, brief);
    }
}
