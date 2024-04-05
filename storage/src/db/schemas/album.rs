use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use surrealqlx::Table;

use crate::util::OneOrMany;

use super::{artist::ArtistId, song::SongId};

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
    /// Ids of the [`Artist`] of this [`Album`] (Can be multiple)
    #[field(dt = "option<set<record> | record>")]
    #[serde(default)]
    pub artist_id: OneOrMany<ArtistId>,
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
    // SOMEDAY:
    // This should be sorted based
    // off incrementing disc and track numbers, e.g:
    //
    // DISC 1:
    //   - 1. ...
    //   - 2. ...
    // DISC 2:
    //   - 1. ...
    //   - 2. ...
    //
    // So, doing `my_album.songs.iter()` will always
    // result in the correct `Song` order for `my_album`.
    /// The [`Id`]s of the [`Song`]s in this [`Album`].
    #[field(dt = "set<record>")]
    pub songs: Box<[SongId]>,
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
