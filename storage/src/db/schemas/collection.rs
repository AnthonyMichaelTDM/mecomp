//! A collection is an auto currated list of similar songs.

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Duration, Id, Thing};
use surrealqlx::Table;

pub type CollectionId = Thing;

pub const TABLE_NAME: &str = "collection";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Table)]
#[Table("collection")]
pub struct Collection {
    /// the unique identifier for this [`Collection`].
    #[field(dt = "record")]
    pub id: CollectionId,

    /// Total runtime.
    #[field(dt = "duration")]
    pub runtime: Duration,

    /// the number of songs this collection has.
    #[field(dt = "int")]
    pub song_count: usize,
}

impl Collection {
    pub fn generate_id() -> CollectionId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionBrief {
    pub id: CollectionId,
    pub runtime: std::time::Duration,
    pub songs: usize,
}

impl From<Collection> for CollectionBrief {
    fn from(collection: Collection) -> Self {
        Self {
            id: collection.id,
            runtime: collection.runtime.into(),
            songs: collection.song_count,
        }
    }
}

impl From<&Collection> for CollectionBrief {
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            runtime: collection.runtime.into(),
            songs: collection.song_count,
        }
    }
}
