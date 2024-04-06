use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Duration, Id, Thing};
use surrealqlx::Table;

pub type ArtistId = Thing;

pub const TABLE_NAME: &str = "artist";

/// This struct holds all the metadata about a particular ['Artist'].
/// An ['Artist'] is a collection of ['Album']s.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Table)]
#[Table("artist")]
pub struct Artist {
    /// the unique identifier for this ['Artist'].
    #[field(dt = "record")]
    pub id: ArtistId,

    /// The [`Artist`]'s name.
    #[field(dt = "string", index(unique))]
    pub name: Arc<str>,

    /// Total runtime.
    #[field(dt = "duration")]
    pub runtime: Duration,

    /// the number of albums this artist has.
    #[field(dt = "int")]
    pub album_count: usize,

    /// the number of songs this artist has.
    #[field(dt = "int")]
    pub song_count: usize,
}

impl Artist {
    pub fn generate_id() -> ArtistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

/// This struct holds all the metadata about a particular ['Artist'].
/// An ['Artist'] is a collection of ['Album']s.
#[derive(Debug, Default, Serialize)]
pub struct ArtistChangeSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: Arc<str>,
    pub runtime: std::time::Duration,
    pub albums: usize,
    pub songs: usize,
}

impl From<Artist> for ArtistBrief {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            runtime: artist.runtime.into(),
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}

impl From<&Artist> for ArtistBrief {
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id.clone(),
            name: artist.name.clone(),
            runtime: artist.runtime.into(),
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}
