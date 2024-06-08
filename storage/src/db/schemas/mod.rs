pub mod album;
pub mod artist;
pub mod collection;
pub mod playlist;
pub mod song;

#[cfg(feature = "db")]
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

#[cfg(feature = "db")]
pub fn serialize_duration_option_as_sql_duration<S>(
    x: &Option<std::time::Duration>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;

    match x {
        Some(x) => serialize_duration_as_sql_duration(x, s),
        None => Option::<surrealdb::sql::Duration>::None.serialize(s),
    }
}

#[cfg(feature = "db")]
pub fn deserialize_duration_from_sql_duration<'de, D>(d: D) -> Result<std::time::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let duration = surrealdb::sql::Duration::deserialize(d)?;
    Ok(duration.into())
}

#[cfg(feature = "db")]
pub fn deserialize_duration_from_sql_duration_option<'de, D>(
    d: D,
) -> Result<Option<std::time::Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let duration = Option::<surrealdb::sql::Duration>::deserialize(d)?;
    Ok(duration.map(Into::into))
}

/// Implement a version of the `surrealdb` `Thing` type that we can use when the `db` feature is not enabled.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thing {
    /// Table name
    pub tb: String,
    pub id: Id,
}

/// Implement a version of the `surrealdb` `Id` type that we can use when the `db` feature is not enabled.
///
/// Only the variants we actually use are implemented.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::module_name_repetitions)]
pub enum Id {
    Number(i64),
    String(String),
}

#[cfg(feature = "db")]
impl From<Thing> for surrealdb::sql::Thing {
    fn from(thing: Thing) -> Self {
        Self {
            tb: thing.tb,
            id: thing.id.into(),
        }
    }
}

#[cfg(feature = "db")]
impl From<Id> for surrealdb::sql::Id {
    fn from(id: Id) -> Self {
        match id {
            Id::Number(n) => Self::Number(n),
            Id::String(s) => Self::String(s),
        }
    }
}

#[cfg(test)]
mod thing {
    //! tests to ensure that the `Thing` type is serialized and deserialized just like the `surrealdb` `Thing` type.
    use super::*;

    #[test]
    fn test_serialize() {
        let thing = Thing {
            tb: "table".to_owned(),
            id: Id::Number(42),
        };
        let serialized = serde_json::to_string(&thing).unwrap();
        let expected = surrealdb::sql::Thing {
            tb: "table".to_owned(),
            id: surrealdb::sql::Id::Number(42),
        };
        let expected = serde_json::to_string(&expected).unwrap();
        assert_eq!(serialized, expected);

        let thing = Thing {
            tb: "table".to_owned(),
            id: Id::String("42".to_owned()),
        };
        let serialized = serde_json::to_string(&thing).unwrap();
        let expected = surrealdb::sql::Thing {
            tb: "table".to_owned(),
            id: surrealdb::sql::Id::String("42".to_owned()),
        };
        let expected = serde_json::to_string(&expected).unwrap();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_deserialize() {
        let serialized = serde_json::to_string(&Thing {
            tb: "table".to_owned(),
            id: Id::String("42".to_owned()),
        })
        .unwrap();

        let thing: Thing = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            thing,
            Thing {
                tb: "table".to_owned(),
                id: Id::String("42".to_owned()),
            }
        );

        let thing: surrealdb::sql::Thing = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            thing,
            surrealdb::sql::Thing {
                tb: "table".to_owned(),
                id: surrealdb::sql::Id::String("42".to_owned()),
            }
        );
    }
}

#[cfg(all(test, feature = "db"))]
mod duration {
    //! tests to ensure that the `std::time::Duration` type is serialized and deserialized just like the `surrealdb` `Duration` type.
    use super::deserialize_duration_from_sql_duration;
    use super::serialize_duration_as_sql_duration;

    #[derive(serde::Serialize, serde::Deserialize)]
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
