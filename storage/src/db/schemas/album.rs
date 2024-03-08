use std::sync::Arc;

use readable::date::Date;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::util::OneOrMany;

use super::{artist::ArtistId, song::SongId};

pub type AlbumId = Thing;

pub const TABLE_NAME: &str = "album";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Album`].
/// An [`Album`] is a collection of [`Song`]s owned by an [`Artist`].
pub struct Album {
    /// The unique identifier for this [`Album`].
    pub id: Option<AlbumId>,
    /// Title of the [`Album`].
    pub title: Arc<str>,
    /// Ids of the [`Artist`] of this [`Album`] (Can be multiple)
    pub artist_id: OneOrMany<ArtistId>,
    /// Artist of the [`Album`]. (Can be multiple)
    pub artist: OneOrMany<Arc<str>>,
    /// Human-readable release date of this [`Album`].
    pub release: Date,
    /// Total runtime of this [`Album`].
    pub runtime: f64,
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
    pub songs: Vec<SongId>,
    /// How many discs are in this `Album`?
    /// (Most will only have 1).
    pub discs: u32,
    /// This [`Album`]'s genre.
    pub genre: Option<OneOrMany<Arc<str>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AlbumBrief {
    pub id: AlbumId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub release: Date,
    pub runtime: f64,
    pub song_count: usize,
    pub discs: u32,
    pub genre: Option<OneOrMany<Arc<str>>>,
}

impl From<Album> for AlbumBrief {
    fn from(album: Album) -> Self {
        Self {
            id: album.id.expect("Album has no id"),
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
