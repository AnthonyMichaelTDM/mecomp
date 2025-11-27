#![allow(clippy::module_name_repetitions)]

use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::RecordId;

use one_or_many::OneOrMany;

pub type AlbumId = RecordId;

pub const TABLE_NAME: &str = "album";

/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`super::song::Song`]s owned by an [`super::artist::Artist`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("album"))]
pub struct Album {
    /// The unique identifier for this [`Album`].
    #[cfg_attr(feature = "db", field(dt = "record"))]
    pub id: AlbumId,
    /// Title of the [`Album`].
    #[cfg_attr(feature = "db", field(dt = "string"))]
    #[cfg_attr(feature = "db", index(text("custom_analyzer")))]
    pub title: String,
    /// Artist of the [`Album`]. (Can be multiple)
    #[cfg_attr(feature = "db", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "db", index(text("custom_analyzer")))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub artist: OneOrMany<String>,
    /// Release year of this [`Album`].
    #[cfg_attr(feature = "db", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub release: Option<u32>,
    /// Total runtime of this [`Album`].
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> {
LET $songs = (SELECT runtime FROM $this.id->album_to_song->song);
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
    /// [`Song`] count of this [`Album`].
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> {
LET $count = (SELECT count() FROM $this.id->album_to_song->song GROUP ALL);
RETURN IF $count IS NONE { 0 } ELSE IF $count.len() == 0 { 0 } ELSE { ($count[0]).count };
}"
        )
    )]
    pub song_count: u64,
    /// How many discs are in this [`Album`]?
    /// (Most will only have 1).
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub discs: u32,
    /// This [`Album`]'s genre.
    #[cfg_attr(feature = "db", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub genre: OneOrMany<String>,
}

impl Album {
    pub const BRIEF_FIELDS: &'static str = "id,title,artist,release,discs,genre";
    #[must_use]
    #[inline]
    pub fn generate_id() -> AlbumId {
        RecordId::from_table_key(TABLE_NAME, Id::ulid())
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AlbumChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub artist: Option<OneOrMany<String>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub release_year: Option<Option<i32>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub discs: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub genre: Option<OneOrMany<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlbumBrief {
    pub id: AlbumId,
    pub title: String,
    pub artist: OneOrMany<String>,
    pub release: Option<u32>,
    pub discs: u32,
    pub genre: OneOrMany<String>,
}

impl From<Album> for AlbumBrief {
    #[inline]
    fn from(album: Album) -> Self {
        Self {
            id: album.id,
            title: album.title,
            artist: album.artist,
            release: album.release,
            discs: album.discs,
            genre: album.genre,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    fn album() -> Album {
        Album {
            id: AlbumId::from((TABLE_NAME, "id")),
            title: "test".into(),
            artist: "test".to_string().into(),
            release: Some(2021),
            runtime: Duration::from_secs(0),
            song_count: 0,
            discs: 1,
            genre: "test".to_string().into(),
        }
    }

    fn album_brief() -> AlbumBrief {
        AlbumBrief {
            id: AlbumId::from((TABLE_NAME, "id")),
            title: "test".into(),
            artist: "test".to_string().into(),
            release: Some(2021),
            discs: 1,
            genre: "test".to_string().into(),
        }
    }

    #[test]
    fn test_album_brief_from_album() {
        let actual: AlbumBrief = album().into();
        assert_eq!(actual, album_brief());
    }
}
