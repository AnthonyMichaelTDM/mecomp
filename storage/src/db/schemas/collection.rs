//! A collection is an auto currated list of similar songs.

use readable::run::Runtime;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::song::SongId;

pub type CollectionId = Thing;

pub const TABLE_NAME: &str = "collection";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Collection {
    /// the unique identifier for this [`Collection`].
    pub id: CollectionId,

    /// Total runtime.
    pub runtime: Runtime,

    /// Keys to every [`Song`] in this [`Collection`].
    pub songs: Box<[SongId]>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionBrief {
    pub id: CollectionId,
    pub runtime: Runtime,
    pub songs: usize,
}

impl From<Collection> for CollectionBrief {
    fn from(collection: Collection) -> Self {
        Self {
            id: collection.id,
            runtime: collection.runtime,
            songs: collection.songs.len(),
        }
    }
}
