#![allow(clippy::module_name_repetitions)]
//! A collection is an auto currated list of similar songs.

use std::sync::Arc;

#[cfg(not(feature = "surrealdb"))]
use crate::surreal::Thing;
#[cfg(feature = "surrealdb")]
use surrealdb::sql::{Duration, Id, Thing};

pub type CollectionId = Thing;

pub const TABLE_NAME: &str = "collection";

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "surrealdb", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "surrealdb", Table("collection"))]
pub struct Collection {
    /// the unique identifier for this [`Collection`].
    #[cfg_attr(feature = "surrealdb", field(dt = "record"))]
    pub id: CollectionId,

    /// The name of the collection.
    #[cfg_attr(feature = "surrealdb", field(dt = "string", index(unique)))]
    pub name: Arc<str>,

    /// Total runtime.
    #[cfg_attr(feature = "surrealdb", field(dt = "duration"))]
    pub runtime: Duration,

    /// the number of songs this collection has.
    #[cfg_attr(feature = "surrealdb", field(dt = "int"))]
    pub song_count: usize,
}

impl Collection {
    #[must_use]
    #[cfg(feature = "surrealdb")]
    pub fn generate_id() -> CollectionId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CollectionChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CollectionBrief {
    pub id: CollectionId,
    pub name: Arc<str>,
    pub runtime: std::time::Duration,
    pub songs: usize,
}

impl From<Collection> for CollectionBrief {
    fn from(collection: Collection) -> Self {
        Self {
            id: collection.id,
            name: collection.name,
            runtime: collection.runtime.into(),
            songs: collection.song_count,
        }
    }
}

impl From<&Collection> for CollectionBrief {
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            name: collection.name.clone(),
            runtime: collection.runtime.into(),
            songs: collection.song_count,
        }
    }
}
