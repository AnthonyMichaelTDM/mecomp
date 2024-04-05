use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use surrealqlx::Table;

use super::{album::AlbumId, song::SongId};

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

    // SOMEDAY:
    // This should be a Box<[AlbumKey]>.
    /// Keys to the associated [`Album`]\(s\).
    #[field(dt = "set<record>")]
    pub albums: Box<[AlbumId]>,

    /// Keys to every [`Song`] by this [`Artist`].
    ///
    /// The order is [`Album`] release order, then [`Song`] track order.
    #[field(dt = "set<record>")]
    pub songs: Box<[SongId]>,
}

impl Artist {
    pub fn generate_id() -> ArtistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: Arc<str>,
    pub runtime: Duration,
    pub albums: usize,
    pub songs: usize,
}

impl From<Artist> for ArtistBrief {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            runtime: artist.runtime,
            albums: artist.albums.len(),
            songs: artist.songs.len(),
        }
    }
}

impl From<&Artist> for ArtistBrief {
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id.clone(),
            name: artist.name.clone(),
            runtime: artist.runtime,
            albums: artist.albums.len(),
            songs: artist.songs.len(),
        }
    }
}
