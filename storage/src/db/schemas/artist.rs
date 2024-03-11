use std::sync::Arc;

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

use super::{album::AlbumId, song::SongId};

pub type ArtistId = Thing;

pub const TABLE_NAME: &str = "artist";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// This struct holds all the metadata about a particular ['Artist'].
/// An ['Artist'] is a collection of ['Album']s.
pub struct Artist {
    /// the unique identifier for this ['Artist'].
    pub id: ArtistId,

    /// The [`Artist`]'s name.
    pub name: Arc<str>,

    /// Total runtime.
    pub runtime: Runtime,

    // SOMEDAY:
    // This should be a Box<[AlbumKey]>.
    /// Keys to the associated [`Album`]\(s\).
    pub albums: Box<[AlbumId]>,

    /// Keys to every [`Song`] by this [`Artist`].
    ///
    /// The order is [`Album`] release order, then [`Song`] track order.
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
    pub runtime: Runtime,
    pub albums: usize,
    pub songs: usize,
}
