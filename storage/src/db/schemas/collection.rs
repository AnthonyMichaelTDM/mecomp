//! A collection is an auto currated list of similar songs.

use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::RecordId;

pub type CollectionId = RecordId;

pub const TABLE_NAME: &str = "collection";

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("collection"))]
pub struct Collection {
    /// the unique identifier for this [`Collection`].
    #[cfg_attr(feature = "db", field("record"))]
    pub id: CollectionId,

    /// The name of the collection.
    #[cfg_attr(feature = "db", field(dt = "string", index(unique)))]
    pub name: String,

    /// Total runtime.
    #[cfg_attr(feature = "db", field(dt = "duration"))]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,

    /// the number of songs this collection has.
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub song_count: usize,
}

impl Collection {
    #[must_use]
    #[inline]
    pub fn generate_id() -> CollectionId {
        RecordId::from_table_key(TABLE_NAME, Id::ulid())
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CollectionChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(
        feature = "db",
        serde(serialize_with = "super::serialize_duration_option_as_sql_duration")
    )]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub song_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CollectionBrief {
    pub id: CollectionId,
    pub name: String,
    pub runtime: std::time::Duration,
    pub songs: usize,
}

impl From<Collection> for CollectionBrief {
    #[inline]
    fn from(collection: Collection) -> Self {
        Self {
            id: collection.id,
            name: collection.name,
            runtime: collection.runtime,
            songs: collection.song_count,
        }
    }
}

impl From<&Collection> for CollectionBrief {
    #[inline]
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            name: collection.name.clone(),
            runtime: collection.runtime,
            songs: collection.song_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn collection() -> Collection {
        Collection {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "collection".into(),
            runtime: Duration::from_secs(3600),
            song_count: 100,
        }
    }

    #[fixture]
    fn collection_brief() -> CollectionBrief {
        CollectionBrief {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "collection".into(),
            runtime: Duration::from_secs(3600),
            songs: 100,
        }
    }

    #[rstest]
    #[case(collection(), collection_brief())]
    #[case(&collection(), collection_brief())]
    fn test_collection_brief_from_collection<T: Into<CollectionBrief>>(
        #[case] collection: T,
        #[case] brief: CollectionBrief,
    ) {
        let actual: CollectionBrief = collection.into();
        assert_eq!(actual, brief);
    }
}
