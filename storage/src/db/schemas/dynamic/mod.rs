#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

#[cfg(not(feature = "db"))]
use super::{Id, Thing};
use query::Query;
#[cfg(feature = "db")]
use surrealdb::sql::{Id, Thing};

pub mod query;

pub type DynamicPlaylistId = Thing;

pub const TABLE_NAME: &str = "dynamic_playlist";

/// This struct holds all the metadata about a particular [`DynamicPlaylist`].
/// A [`DynamicPlaylist`] is essentially a query that returns a list of [`super::song::Song`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("playlist"))]
pub struct DynamicPlaylist {
    /// the unique identifier for this [`DynamicPlaylist`].
    #[cfg_attr(feature = "db", field("any"))]
    pub id: DynamicPlaylistId,

    /// The [`DynamicPlaylist`]'s name.
    #[cfg_attr(feature = "db", field(dt = "string", index(unique)))]
    pub name: Arc<str>,

    /// The query that generates the list of songs.
    /// This is a type that can compile into an SQL query that returns a list of song IDs.
    #[cfg_attr(feature = "db", field(dt = "object"))]
    pub query: Query,
}

impl DynamicPlaylist {
    #[must_use]
    pub fn generate_id() -> DynamicPlaylistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DynamicPlaylistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub query: Option<Query>,
}
