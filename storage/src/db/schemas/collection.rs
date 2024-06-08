#![allow(clippy::module_name_repetitions)]
//! A collection is an auto currated list of similar songs.

use std::sync::Arc;

#[cfg(not(any(test, feature = "db")))]
use super::Thing;
#[cfg(not(any(test, feature = "db")))]
use std::time::Duration;
#[cfg(any(test, feature = "db"))]
use surrealdb::sql::{Duration, Id, Thing};

pub type CollectionId = Thing;

pub const TABLE_NAME: &str = "collection";

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "db"), derive(surrealqlx::Table))]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(test, feature = "db"), Table("collection"))]
pub struct Collection {
    /// the unique identifier for this [`Collection`].
    #[cfg_attr(any(test, feature = "db"), field(dt = "record"))]
    pub id: CollectionId,

    /// The name of the collection.
    #[cfg_attr(any(test, feature = "db"), field(dt = "string", index(unique)))]
    pub name: Arc<str>,

    /// Total runtime.
    #[cfg_attr(any(test, feature = "db"), field(dt = "duration"))]
    pub runtime: Duration,

    /// the number of songs this collection has.
    #[cfg_attr(any(test, feature = "db"), field(dt = "int"))]
    pub song_count: usize,
}

impl Collection {
    #[must_use]
    #[cfg(any(test, feature = "db"))]
    pub fn generate_id() -> CollectionId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(any(test, feature = "serde"), derive(serde::Serialize))]
pub struct CollectionChangeSet {
    #[cfg_attr(
        any(test, feature = "serde"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub name: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
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
            #[cfg(not(any(test, feature = "db")))]
            runtime: collection.runtime,
            #[cfg(any(test, feature = "db"))]
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
            #[cfg(not(any(test, feature = "db")))]
            runtime: collection.runtime,
            #[cfg(any(test, feature = "db"))]
            runtime: collection.runtime.into(),
            songs: collection.song_count,
        }
    }
}
