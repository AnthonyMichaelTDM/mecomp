//! A collection is an auto currated list of similar songs.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use surrealqlx::Table;

use super::song::SongId;

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

    /// Keys to every [`Song`] in this [`Collection`].
    #[field(dt = "set<record>")]
    pub songs: Box<[SongId]>,
}

impl Collection {
    pub fn generate_id() -> CollectionId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionBrief {
    pub id: CollectionId,
    pub runtime: Duration,
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

impl From<&Collection> for CollectionBrief {
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            runtime: collection.runtime,
            songs: collection.songs.len(),
        }
    }
}
