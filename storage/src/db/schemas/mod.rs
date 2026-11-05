#![allow(clippy::module_name_repetitions)]
use std::str::FromStr;

use crate::errors::InvalidId;

pub mod album;
#[cfg(feature = "analysis")]
pub mod analysis;
pub mod artist;
pub mod collection;
pub mod dynamic;
pub mod playlist;
pub mod song;

/// Serialize a `std::time::Duration` as a `surrealdb::sql::Duration`.
///
/// # Errors
///
/// This function will return an error if the `std::time::Duration` cannot be serialized as a `surrealdb::sql::Duration`.
#[cfg(feature = "db")]
#[inline]
pub fn serialize_duration_as_sql_duration<S>(
    x: &std::time::Duration,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;

    Into::<surrealdb::sql::Duration>::into(*x).serialize(s)
}

/// Serialize an `Option<std::time::Duration>` as an `Option<surrealdb::sql::Duration>`.
///
/// # Errors
///
/// This function will return an error if the `Option<std::time::Duration>` cannot be serialized as an `Option<surrealdb::sql::Duration>`.
#[cfg(feature = "db")]
#[inline]
pub fn serialize_duration_option_as_sql_duration<S>(
    x: &Option<std::time::Duration>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;

    x.map(Into::<surrealdb::sql::Duration>::into).serialize(s)
}

/// Deserialize a `std::time::Duration` from a `surrealdb::sql::Duration`.
///
/// # Errors
///
/// This function will return an error if the `std::time::Duration` cannot be deserialized from a `surrealdb::sql::Duration`.
#[cfg(feature = "db")]
#[inline]
pub fn deserialize_duration_from_sql_duration<'de, D>(d: D) -> Result<std::time::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let duration = surrealdb::sql::Duration::deserialize(d)?;
    Ok(duration.into())
}

/// Implement a version of the `surrealdb` `RecordId` type that we can use when the `db` feature is not enabled.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "$surrealdb::private::sql::Thing"))]
pub struct RecordId {
    /// Table name
    pub tb: String,
    pub id: Id,
}

impl std::fmt::Debug for RecordId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.tb, self.id)
    }
}

impl std::fmt::Display for RecordId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.tb, self.id)
    }
}

impl<S: Into<String>, I: Into<Id>> From<(S, I)> for RecordId {
    #[inline]
    fn from((tb, id): (S, I)) -> Self {
        Self {
            tb: tb.into(),
            id: id.into(),
        }
    }
}

impl RecordId {
    /// Get the table name.
    #[must_use]
    #[inline]
    #[allow(clippy::missing_const_for_fn)] // TODO: make this const when possible
    pub fn table(&self) -> &str {
        &self.tb
    }

    /// Get the id.
    #[must_use]
    #[inline]
    pub const fn key(&self) -> &Id {
        &self.id
    }

    /// Create a new `RecordId` from a table name and an id.
    #[must_use]
    #[inline]
    pub fn from_table_key<S, K>(table: S, key: K) -> Self
    where
        S: Into<String>,
        K: Into<Id>,
    {
        Self {
            tb: table.into(),
            id: key.into(),
        }
    }
}

impl FromStr for RecordId {
    type Err = InvalidId;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // deserialize the thing from the string
        // the line should follow the pattern:
        // <table_name>:<26 character, upperalphanumeric id>
        // anything else should be considered invalid, and ignored
        //
        // input may also look like:
        //     <table_name>:<26 character, upperalphanumeric id>: <some other text>
        // this is okay too, the extra text will be ignored
        let parts: Vec<&str> = s.trim().split(':').collect();

        if parts.len() >= 2
            && (matches!(
                parts[0],
                artist::TABLE_NAME
                    | album::TABLE_NAME
                    | song::TABLE_NAME
                    | playlist::TABLE_NAME
                    | collection::TABLE_NAME
                    | dynamic::TABLE_NAME
            ))
        {
            let id = Id::from_str(parts[1])?;
            Ok(Self {
                tb: parts[0].to_owned(),
                id,
            })
        } else {
            Err(InvalidId)
        }
    }
}

/// Implement a version of the `surrealdb` `Id` type that we can use when the `db` feature is not enabled.
///
/// Only the variants we actually use are implemented.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Id {
    Number(i64),
    String(String),
}

impl Id {
    /// Generate a new `Id::String` variant from a `Ulid`.
    #[must_use]
    #[inline]
    pub fn ulid() -> Self {
        Self::String(ulid::Ulid::new().to_string())
    }
}

impl std::fmt::Display for Id {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for Id {
    type Err = InvalidId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && s.len() == 26
        {
            Ok(Self::String(s.to_owned()))
        } else {
            Err(InvalidId)
        }
    }
}

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

#[cfg(feature = "db")]
impl From<Id> for surrealdb::RecordIdKey {
    #[inline]
    fn from(id: Id) -> Self {
        match id {
            Id::Number(n) => Self::from(n),
            Id::String(s) => Self::from(s),
        }
    }
}

#[cfg(feature = "db")]
impl From<RecordId> for surrealdb::RecordId {
    #[inline]
    fn from(thing: RecordId) -> Self {
        Self::from_table_key(thing.tb, surrealdb::RecordIdKey::from(thing.id))
    }
}

// Implementations to allow our [`RecordId`] type to be used like a normal [`surrealdb::RecordId`] in SurrealDB queries.

#[cfg(feature = "db")]
impl<R> surrealdb::opt::IntoResource<Option<R>> for &RecordId {
    fn into_resource(self) -> Result<surrealdb::opt::Resource, surrealdb::Error> {
        let surreal_recid: surrealdb::RecordId = self.clone().into();
        Ok(surreal_recid.into())
    }
}

#[cfg(feature = "db")]
impl<R> surrealdb::opt::CreateResource<Option<R>> for &RecordId {
    fn into_resource(self) -> Result<surrealdb::opt::Resource, surrealdb::Error> {
        let surreal_recid: surrealdb::RecordId = self.clone().into();
        Ok(surreal_recid.into())
    }
}

#[cfg(test)]
mod thing {
    //! tests to ensure that the `RecordId` type is serialized and deserialized just like the `surrealdb` `RecordId` type.
    use crate::errors::InvalidId;

    use super::*;

    #[test]
    fn test_serialize() {
        let thing = RecordId {
            tb: "table".to_owned(),
            id: Id::Number(42),
        };
        let serialized = serde_json::to_string(&thing).unwrap();
        let expected = surrealdb::RecordId::from(("table", 42));

        let expected = serde_json::to_string(&expected).unwrap();
        assert_eq!(serialized, expected);

        let thing = RecordId {
            tb: "table".to_owned(),
            id: Id::String("42".to_owned()),
        };
        let serialized = serde_json::to_string(&thing).unwrap();
        let expected = surrealdb::RecordId::from(("table", "42"));
        let expected = serde_json::to_string(&expected).unwrap();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_deserialize() {
        let serialized = serde_json::to_string(&RecordId {
            tb: "table".to_owned(),
            id: Id::String("42".to_owned()),
        })
        .unwrap();

        let thing: RecordId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            thing,
            RecordId {
                tb: "table".to_owned(),
                id: Id::String("42".to_owned()),
            }
        );

        let thing: surrealdb::RecordId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(thing, surrealdb::RecordId::from_table_key("table", "42"));
    }

    #[test]
    fn test_from_str() {
        let id = Id::ulid();

        // valid things
        let thing: RecordId = format!("song:{id}").parse().unwrap();
        assert_eq!(thing, RecordId::from(("song", id.clone())));
        let thing: RecordId = format!("song:{id}: extra text").parse().unwrap();
        assert_eq!(thing, RecordId::from(("song", id.clone())));

        // id too short
        let thing: Result<RecordId, InvalidId> = "song:42".parse();
        assert_eq!(thing.unwrap_err(), InvalidId);
        let thing: Result<RecordId, InvalidId> = "song:42:extra text:".parse();
        assert_eq!(thing.unwrap_err(), InvalidId);

        // id too long
        let thing: Result<RecordId, InvalidId> = format!("song:{}", "a".repeat(27)).parse();
        assert_eq!(thing.unwrap_err(), InvalidId);
        let thing: Result<RecordId, InvalidId> =
            format!("song:{}: extra text", "a".repeat(27)).parse();
        assert_eq!(thing.unwrap_err(), InvalidId);

        // extra text without colon
        let thing: Result<RecordId, InvalidId> = format!("song:{id} extra text").parse();
        assert_eq!(thing.unwrap_err(), InvalidId);

        // invalid table name
        let thing: Result<RecordId, InvalidId> = format!("table:{id}").parse();
        assert_eq!(thing.unwrap_err(), InvalidId);
        let thing: Result<RecordId, InvalidId> = format!("table:{id}: extra text").parse();
        assert_eq!(thing.unwrap_err(), InvalidId);

        // text is not a id at all
        let thing: Result<RecordId, InvalidId> = "hello world!".parse();
        assert_eq!(thing.unwrap_err(), InvalidId);
    }
}

#[cfg(all(test, feature = "db"))]
mod duration {
    //! tests to ensure that the `std::time::Duration` type is serialized and deserialized just like the `surrealdb` `Duration` type.
    use super::deserialize_duration_from_sql_duration;
    use super::serialize_duration_as_sql_duration;

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(dead_code)]
    struct TestStruct {
        #[serde(
            serialize_with = "serialize_duration_as_sql_duration",
            deserialize_with = "deserialize_duration_from_sql_duration"
        )]
        duration: std::time::Duration,
    }

    #[test]
    fn test_serialize() {
        let duration = std::time::Duration::from_secs(42);
        let serialized = serde_json::to_string(&duration).unwrap();
        let expected = surrealdb::sql::Duration::from_secs(42);
        let expected = serde_json::to_string(&expected).unwrap();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_deserialize() {
        let serialized = serde_json::to_string(&surrealdb::sql::Duration::from_secs(42)).unwrap();

        let duration: std::time::Duration = serde_json::from_str(&serialized).unwrap();
        assert_eq!(duration, std::time::Duration::from_secs(42));

        let duration: surrealdb::sql::Duration = serde_json::from_str(&serialized).unwrap();
        assert_eq!(duration, surrealdb::sql::Duration::from_secs(42));
    }
}
