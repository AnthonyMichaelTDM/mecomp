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
    #[cfg_attr(feature = "db", field(dt = "record"))]
    pub id: PlaylistId,

    /// The [`Playlist`]'s name.
    #[cfg_attr(feature = "db", field(dt = "string"))]
    #[cfg_attr(feature = "db", index(unique))]
    pub name: String,

    /// Total runtime.
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> {
LET $songs = (SELECT runtime FROM $this.id->playlist_to_song->song);
RETURN IF $songs IS NONE { 0s } ELSE { $songs.fold(0s, |$acc, $song| $acc + $song.runtime) };
}"
        )
    )]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,

    /// the number of songs this playlist has.
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> { 
LET $count = (SELECT count() FROM $this.id->playlist_to_song->song GROUP ALL);
RETURN IF $count IS NONE { 0 } ELSE IF $count.len() == 0 { 0 } ELSE { ($count[0]).count };
}"
        )
    )]
    pub song_count: u64,
}

impl Playlist {
    pub const BRIEF_FIELDS: &'static [&'static str] = &["id", "name"];

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistBrief {
    pub id: PlaylistId,
    pub name: String,
}

impl From<Playlist> for PlaylistBrief {
    #[inline]
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::fixture;

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
        }
    }

    #[test]
    fn test_playlist_brief_from_playlist() {
        let actual: PlaylistBrief = playlist().into();
        assert_eq!(actual, playlist_brief());
    }
}
