//! Implement a version of the `surrealdb` `Thing` type that we can use when the `surrealdb` feature is not enabled.
//!
//! Only the variants we actually use are implemented.

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thing {
    /// Table name
    pub tb: String,
    pub id: Id,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::module_name_repetitions)]
pub enum Id {
    Number(i64),
    String(String),
}

#[cfg(feature = "surrealdb")]
impl From<Thing> for surrealdb::sql::Thing {
    fn from(thing: Thing) -> Self {
        Self {
            tb: thing.tb,
            id: thing.id.into(),
        }
    }
}

#[cfg(feature = "surrealdb")]
impl From<Id> for surrealdb::sql::Id {
    fn from(id: Id) -> Self {
        match id {
            Id::Number(n) => Self::Number(n),
            Id::String(s) => Self::String(s),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
