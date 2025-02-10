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
            // This query would make "artist ANYINSIDE ['foo', 'bar']" type queries work, but breaks almost everything else
            // "SELECT * FROM (SELECT id, title, album, track, disc, path, extension, release_year, runtime, array::flatten([artist][? $this]) AS artist, array::flatten([album_artist][? $this]) AS album_artist, array::flatten([genre][? $this]) AS genre FROM {table_name}) WHERE {conditions};",
            "SELECT * FROM {table_name} WHERE {conditions};",
            table_name = super::song::TABLE_NAME,
            conditions = self.query.compile(query::Context::Execution)
        )
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    use pretty_assertions::assert_eq;
    use query::{Clause, CompoundClause, CompoundKind, Field, LeafClause, Operator, Value};
    use rstest::rstest;

    #[rstest]
    #[case::leaf_clause(
        Query {root: Clause::Leaf(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::String("foo".to_string())
        })},
        "SELECT * FROM song WHERE title = 'foo';"
    )]
    #[case::leaf_clause(
        Query { root: Clause::Leaf(LeafClause {
            left: Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)]),
            operator: Operator::Contains,
            right: Value::Int(42)
        })},
        "SELECT * FROM song WHERE [\"foo\", 42] CONTAINS 42;"
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
                    operator: Operator::Contains,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::And
        })},
        "SELECT * FROM song WHERE (title = \"foo\" AND array::flatten([artist][? $this]) CONTAINS \"bar\");"
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
                    operator: Operator::Contains,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::Or
        })},
        "SELECT * FROM song WHERE (title = \"foo\" OR array::flatten([artist][? $this]) CONTAINS \"bar\");"
    )]
    #[case::query(
        Query {
            root: Clause::Compound(CompoundClause {
                clauses: vec![
                    Clause::Compound(
                        CompoundClause {
                            clauses: vec![
                                Clause::Leaf(LeafClause {
                                    left: Value::Field(Field::Artists),
                                    operator: Operator::AnyInside,
                                    right: Value::Set(vec![Value::String("foo".to_string()), Value::String("bar".to_string())])
                                }),
                                Clause::Compound(CompoundClause {
                                    clauses: vec![
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::AlbumArtists),
                                            operator: Operator::Contains,
                                            right: Value::String("bar".to_string())
                                        }),
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::Genre),
                                            operator: Operator::AnyLike,
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
                        left: Value::Field(Field::ReleaseYear),
                        operator: Operator::GreaterThan,
                        right: Value::Int(2020)
                    }),
                ],
                kind: CompoundKind::And
            })
        },
        "SELECT * FROM song WHERE ((array::flatten([artist][? $this]) ANYINSIDE [\"foo\", \"bar\"] AND (array::flatten([album_artist][? $this]) CONTAINS \"bar\" OR array::flatten([genre][? $this])  ?~ \"baz\")) AND release_year > 2020);"
    )]
    fn test_compile(#[case] query: Query, #[case] expected: impl IntoQuery) {
        let dynamic_playlist = DynamicPlaylist {
            id: DynamicPlaylist::generate_id(),
            name: Arc::from("test"),
            query,
        };

        let compiled = dynamic_playlist.get_query().into_query();

        assert!(compiled.is_ok());
        assert_eq!(compiled.unwrap(), expected.into_query().unwrap());
    }
}
