//----------------------------------------------------------------------------------------- std lib
use std::path::PathBuf;
use std::sync::Arc;
//--------------------------------------------------------------------------------- other libraries
use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
//----------------------------------------------------------------------------------- local modules
use super::{album::AlbumId, artist::ArtistId};
use crate::util::OneOrMany;

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Song`].
pub struct Song {
    // / The unique identifier for this [`Song`].
    pub id: Option<SongId>,
    /// Title of the [`Song`].
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist_ids: OneOrMany<ArtistId>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artists: OneOrMany<Arc<str>>,
    /// Key to the [`Album`].
    pub album_id: AlbumId,
    /// album title
    pub album: Arc<str>,
    /// Genre of the [`Song`]. (Can be multiple)
    pub genre: Option<OneOrMany<Arc<str>>>,

    /// Total runtime of this [`Song`].
    pub duration: Runtime,
    /// Sample rate of this [`Song`].
    pub sample_rate: u32,
    /// The track number of this [`Song`].
    pub track: Option<u32>,
    /// The disc number of this [`Song`].
    pub disc: Option<u32>,

    /// The `MIME` type of this [`Song`].
    pub mime: Arc<str>,
    /// The file extension of this [`Song`].
    pub extension: Arc<str>,

    /// The [`PathBuf`] this [`Song`] is located at.
    pub path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SongBrief {
    pub id: SongId,
    pub title: Arc<str>,
    pub artists: OneOrMany<Arc<str>>,
    pub album: Arc<str>,
    pub duration: Runtime,
    pub path: PathBuf,
}

impl From<Song> for SongBrief {
    fn from(song: Song) -> Self {
        Self {
            id: song.id.expect("Song has no id"),
            title: song.title,
            artists: song.artists,
            album: song.album,
            duration: song.duration,
            path: song.path,
        }
    }
}
