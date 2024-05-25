use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement, UpdateStatement},
    Data, Ident, Idiom, Operator, Param, Part, Value, Values,
};

use super::generic::{read_related_out, relate, unrelate};

/// Query to relate a collection to its songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->collection_to_song->$songs
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> RelateStatement {
    relate("id", "songs", "collection_to_song")
}

/// Query to read the songs of a collection
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->collection_to_song.out
/// ```
#[must_use]
#[inline]
pub fn read_songs() -> SelectStatement {
    read_related_out("id", "collection_to_song")
}

/// Query to remove songs from a collection
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $id->collection_to_song WHERE out IN $songs
/// ```
#[must_use]
#[inline]
pub fn remove_songs() -> DeleteStatement {
    unrelate("id", "songs", "collection_to_song")
}

/// Query to "repair" a collection.
///
/// This query updates the `song_count` and runtime of the collection.
///
/// Compiles to:
/// ```sql, ignore
/// UPDATE $id SET song_count=$songs, runtime=$runtime
/// ```
#[must_use]
pub fn repair() -> UpdateStatement {
    UpdateStatement {
        what: Values(vec![Value::Param(Param(Ident("id".into())))]),
        data: Some(Data::SetExpression(vec![
            (
                Idiom(vec![Part::Field(Ident("song_count".into()))]),
                Operator::Equal,
                Value::Param(Param(Ident("songs".into()))),
            ),
            (
                Idiom(vec![Part::Field(Ident("runtime".into()))]),
                Operator::Equal,
                Value::Param(Param(Ident("runtime".into()))),
            ),
        ])),
        ..Default::default()
    }
}
