use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use surrealqlx::Table;

use crate::util::OneOrMany;

pub type AlbumId = Thing;

pub const TABLE_NAME: &str = "album";

/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Table)]
#[Table("album")]
pub struct Album {
    /// The unique identifier for this [`Album`].
    #[field(dt = "record")]
    pub id: AlbumId,
    /// Title of the [`Album`].
    #[field(dt = "string", index())]
    pub title: Arc<str>,
    /// Artist of the [`Album`]. (Can be multiple)
    #[field(dt = "option<set<string> | string>")]
    #[serde(default)]
    pub artist: OneOrMany<Arc<str>>,
    /// Release year of this [`Album`].
    #[field(dt = "option<int>")]
    #[serde(default)]
    pub release: Option<i32>,
    /// Total runtime of this [`Album`].
    #[field(dt = "duration")]
    pub runtime: Duration,
    /// [`Song`] count of this [`Album`].
    #[field(dt = "int")]
    pub song_count: usize,
    /// How many discs are in this `Album`?
    /// (Most will only have 1).
    #[field(dt = "int")]
    pub discs: u32,
    /// This [`Album`]'s genre.
    #[field(dt = "option<set<string> | string>")]
    #[serde(default)]
    pub genre: OneOrMany<Arc<str>>,
}

impl Album {
    pub fn generate_id() -> AlbumId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default, Serialize)]
pub struct AlbumChangeSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<OneOrMany<Arc<str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<Option<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<OneOrMany<Arc<str>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
