#![allow(clippy::module_name_repetitions)]
use std::sync::Arc;

#[cfg(not(feature = "db"))]
use super::{Id, Thing};
use query::Query;
#[cfg(feature = "db")]
use surrealdb::{
    opt::IntoQuery,
    sql::{Id, Thing},
};

pub mod query;

pub type DynamicPlaylistId = Thing;

pub const TABLE_NAME: &str = "dynamic";

/// This struct holds all the metadata about a particular [`DynamicPlaylist`].
/// A [`DynamicPlaylist`] is essentially a query that returns a list of [`super::song::Song`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("dynamic"))]
pub struct DynamicPlaylist {
    /// the unique identifier for this [`DynamicPlaylist`].
    #[cfg_attr(feature = "db", field("any"))]
    pub id: DynamicPlaylistId,

    /// The [`DynamicPlaylist`]'s name.
    #[cfg_attr(feature = "db", field(dt = "string", index(unique)))]
    pub name: Arc<str>,

    /// The query that generates the list of songs.
    /// This is a type that can compile into an SQL query that returns a list of song IDs.
    /// NOTE: we store it as the compiled string because `SurrealDB` wasn't storing records properly
    #[cfg_attr(feature = "db", field("string"))]
    pub query: Query,
}

impl DynamicPlaylist {
    #[must_use]
    pub fn generate_id() -> DynamicPlaylistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }

    #[must_use]
    #[cfg(feature = "db")]
    pub fn get_query(&self) -> impl IntoQuery {
        use query::Compile;

        format!(
            "SELECT * FROM {table_name} WHERE {conditions}",
            table_name = super::song::TABLE_NAME,
            conditions = self.query.compile()
        )
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DynamicPlaylistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<Arc<str>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub query: Option<Query>,
}

impl DynamicPlaylistChangeSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<Arc<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn query(mut self, query: Query) -> Self {
        self.query = Some(query);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = DynamicPlaylist::generate_id();
        assert_eq!(id.tb, TABLE_NAME);
    }
}

#[cfg(all(test, feature = "db"))]
mod query_tests {
    use super::*;
    use query::{Clause, CompoundClause, CompoundKind, Field, LeafClause, Operator, Value};
    use rstest::rstest;

    #[rstest]
    #[case::leaf_clause(
        Query {root: Clause::Leaf(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::String("foo".to_string())
        })},
    )]
    #[case::leaf_clause(
        Query { root: Clause::Leaf(LeafClause {
            left: Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)]),
            operator: Operator::Contains,
            right: Value::Int(42)
        })},
    )]
    #[case::compound_clause(
        Query { root: Clause::Compound( CompoundClause {
            clauses: vec![
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Title),
                    operator: Operator::Equal,
                    right: Value::String("foo".to_string())
                }),
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Artists),
                    operator: Operator::Equal,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::And
        })},
    )]
    #[case::compound_clause(
        Query { root: Clause::Compound(CompoundClause {
            clauses: vec![
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Title),
                    operator: Operator::Equal,
                    right: Value::String("foo".to_string())
                }),
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Artists),
                    operator: Operator::Equal,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::Or
        })},
    )]
    #[case::query(
        Query {
            root: Clause::Compound(CompoundClause {
                clauses: vec![
                    Clause::Compound(
                        CompoundClause {
                            clauses: vec![
                                Clause::Leaf(LeafClause {
                                    left: Value::Field(Field::Title),
                                    operator: Operator::Equal,
                                    right: Value::String("foo".to_string())
                                }),
                                Clause::Compound(CompoundClause {
                                    clauses: vec![
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::Artists),
                                            operator: Operator::Equal,
                                            right: Value::String("bar".to_string())
                                        }),
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::Album),
                                            operator: Operator::Equal,
                                            right: Value::String("baz".to_string())
                                        }),
                                    ],
                                    kind: CompoundKind::Or
                                }),
                            ],
                            kind: CompoundKind::And
                        }
                    ),
                    Clause::Leaf(LeafClause {
                        left: Value::Field(Field::Year),
                        operator: Operator::GreaterThan,
                        right: Value::Int(2020)
                    }),
                ],
                kind: CompoundKind::And
            })
        },
    )]
    fn test_compile(#[case] query: Query) {
        let dynamic_playlist = DynamicPlaylist {
            id: DynamicPlaylist::generate_id(),
            name: Arc::from("test"),
            query,
        };

        assert!(dynamic_playlist.get_query().into_query().is_ok());
    }
}
