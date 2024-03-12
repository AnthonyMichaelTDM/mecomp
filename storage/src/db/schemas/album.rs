use std::sync::Arc;

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

use crate::util::OneOrMany;

use super::{artist::ArtistId, song::SongId};

pub type AlbumId = Thing;

pub const TABLE_NAME: &str = "album";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
pub struct Album {
    /// The unique identifier for this [`Album`].
    pub id: AlbumId,
    /// Title of the [`Album`].
    pub title: Arc<str>,
    /// Ids of the [`Artist`] of this [`Album`] (Can be multiple)
    pub artist_id: OneOrMany<ArtistId>,
    /// Artist of the [`Album`]. (Can be multiple)
    pub artist: OneOrMany<Arc<str>>,
    /// Release year of this [`Album`].
    pub release: Option<i32>,
    /// Total runtime of this [`Album`].
    pub runtime: Runtime,
    /// [`Song`] count of this [`Album`].
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
    pub songs: Box<[SongId]>,
    /// How many discs are in this `Album`?
    /// (Most will only have 1).
    pub discs: u32,
    /// This [`Album`]'s genre.
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
    pub runtime: Runtime,
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
