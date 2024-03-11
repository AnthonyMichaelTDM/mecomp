//----------------------------------------------------------------------------------------- std lib
use audiotags::Config;
use std::path::PathBuf;
use std::sync::Arc;
//--------------------------------------------------------------------------------- other libraries
use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
//----------------------------------------------------------------------------------- local modules
use super::{
    album::{Album, AlbumId},
    artist::{Artist, ArtistId},
};
use crate::{
    errors::{Error, SongIOError},
    util::OneOrMany,
};

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Song`].
pub struct Song {
    // / The unique identifier for this [`Song`].
    pub id: SongId,
    /// Title of the [`Song`].
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist_id: OneOrMany<ArtistId>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist: OneOrMany<Arc<str>>,
    /// album artist, if not found then defaults to first artist
    pub album_artist: OneOrMany<Arc<str>>,
    /// album artist id
    pub album_artist_id: OneOrMany<ArtistId>,

    /// Key to the [`Album`].
    pub album_id: AlbumId,
    /// album title
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    pub genre: Option<OneOrMany<Arc<str>>>,

    /// Total runtime of this [`Song`].
    pub duration: Runtime,
    // /// Sample rate of this [`Song`].
    // pub sample_rate: u32,
    /// The track number of this [`Song`].
    pub track: Option<u16>,
    /// The disc number of this [`Song`].
    pub disc: Option<u16>,

    // /// The `MIME` type of this [`Song`].
    // pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    pub path: PathBuf,
}

impl Song {
    pub fn generate_id() -> SongId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SongBrief {
    pub id: SongId,
    pub title: Arc<str>,
    pub artist: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub album_artist: OneOrMany<Arc<str>>,
    pub duration: Runtime,
    pub path: PathBuf,
}

impl From<Song> for SongBrief {
    fn from(song: Song) -> Self {
        Self {
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.album_artist,
            duration: song.duration,
            path: song.path,
        }
    }
}
