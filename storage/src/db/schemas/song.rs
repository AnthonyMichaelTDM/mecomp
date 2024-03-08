//----------------------------------------------------------------------------------------- std lib
use std::path::PathBuf;
use std::sync::Arc;
//--------------------------------------------------------------------------------- other libraries
use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::util::OneOrMany;
//----------------------------------------------------------------------------------- local modules
use super::album::AlbumId;

pub type SongId = Thing;

pub const TABLE_NAME: &str = "song";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular [`Song`].
pub struct Song {
    // / The unique identifier for this [`Song`].
    // pub id: Option<SongId>,
    /// Title of the [`Song`].
    pub title: Arc<str>,
    /// Artist of the [`Song`]. (Can be multiple)
    pub artist: OneOrMany<Arc<str>>,
    /// Key to the [`Album`].
    pub album: AlbumId,
    /// album title
    pub album_title: Arc<str>,
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
