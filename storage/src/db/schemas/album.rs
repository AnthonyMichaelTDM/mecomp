#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

#[cfg(not(feature = "db"))]
use super::Thing;
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::sql::{Id, Thing};

use one_or_many::OneOrMany;

pub type AlbumId = Thing;

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
    #[cfg_attr(feature = "db", field(dt = "string", index(text("custom_analyzer"))))]
    pub title: Arc<str>,
    /// Artist of the [`Album`]. (Can be multiple)
    #[cfg_attr(
        feature = "db",
        field(dt = "option<set<string> | string>", index(text("custom_analyzer")))
    )]
    #[cfg_attr(feature = "serde", serde(default))]
    pub artist: OneOrMany<Arc<str>>,
    /// Release year of this [`Album`].
    #[cfg_attr(feature = "db", field(dt = "option<int>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub release: Option<i32>,
    /// Total runtime of this [`Album`].
    #[cfg_attr(feature = "db", field(dt = "duration"))]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,
    /// [`Song`] count of this [`Album`].
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub song_count: usize,
    /// How many discs are in this [`Album`]?
    /// (Most will only have 1).
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub discs: u32,
    /// This [`Album`]'s genre.
    #[cfg_attr(feature = "db", field(dt = "option<set<string> | string>"))]
    #[cfg_attr(feature = "serde", serde(default))]
    pub genre: OneOrMany<Arc<str>>,
}

impl Album {
    #[must_use]
    #[cfg(feature = "db")]
    pub fn generate_id() -> AlbumId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AlbumChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub artist: Option<OneOrMany<Arc<str>>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub release: Option<Option<i32>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(
        feature = "db",
        serde(serialize_with = "super::serialize_duration_option_as_sql_duration",)
    )]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub song_count: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub discs: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub genre: Option<OneOrMany<Arc<str>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlbumBrief {
    pub id: AlbumId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub release: Option<i32>,
    pub runtime: Duration,
    pub song_count: usize,
    pub discs: u32,
    pub genre: OneOrMany<Arc<str>>,
}

impl From<Album> for AlbumBrief {
    fn from(album: Album) -> Self {
        Self {
            id: album.id,
            title: album.title,
            artist: album.artist,
            release: album.release,
            runtime: album.runtime,
            song_count: album.song_count,
            discs: album.discs,
            genre: album.genre,
        }
    }
}

impl From<&Album> for AlbumBrief {
    fn from(album: &Album) -> Self {
        Self {
            id: album.id.clone(),
            title: album.title.clone(),
            artist: album.artist.clone(),
            release: album.release,
            runtime: album.runtime,
            song_count: album.song_count,
            discs: album.discs,
            genre: album.genre.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn album() -> Album {
        Album {
            id: AlbumId::from((TABLE_NAME, "id")),
            title: Arc::from("test"),
            artist: OneOrMany::One(Arc::from("test")),
            release: Some(2021),
            runtime: Duration::from_secs(0),
            song_count: 0,
            discs: 1,
            genre: OneOrMany::One(Arc::from("test")),
        }
    }

    #[fixture]
    fn album_brief() -> AlbumBrief {
        AlbumBrief {
            id: AlbumId::from((TABLE_NAME, "id")),
            title: Arc::from("test"),
            artist: OneOrMany::One(Arc::from("test")),
            release: Some(2021),
            runtime: Duration::from_secs(0),
            song_count: 0,
            discs: 1,
            genre: OneOrMany::One(Arc::from("test")),
        }
    }

    #[rstest]
    #[case(album(), album_brief())]
    #[case(&album(), album_brief())]
    fn test_album_brief_from_album<T: Into<AlbumBrief>>(
        #[case] album: T,
        #[case] brief: AlbumBrief,
    ) {
        let actual: AlbumBrief = album.into();
        assert_eq!(actual, brief);
    }
}
