use std::sync::Arc;

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::{album::AlbumId, song::SongId};

pub type ArtistId = Thing;

pub const TABLE_NAME: &str = "artist";

#[derive(Clone, Debug, Deserialize, Serialize)]
/// This struct holds all the metadata about a particular ['Artist'].
/// An ['Artist'] is a collection of ['Album']s.
pub struct Artist {
    /// the unique identifier for this ['Artist'].
    pub id: Option<ArtistId>,

    /// The [`Artist`]'s name.
    pub name: Arc<str>,

    /// Total runtime.
    pub runtime: Runtime,

    // SOMEDAY:
    // This should be a Box<[AlbumKey]>.
    /// Keys to the associated [`Album`]\(s\).
    pub albums: Vec<AlbumId>,

    /// Keys to every [`Song`] by this [`Artist`].
    ///
    /// The order is [`Album`] release order, then [`Song`] track order.
    pub songs: Vec<SongId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: Arc<str>,
    pub runtime: Runtime,
    pub albums: usize,
    pub songs: usize,
}
