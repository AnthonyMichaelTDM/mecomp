use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement, UpdateStatement},
    Data, Ident, Idiom, Operator, Param, Part, Value, Values,
};

use super::generic::{read_related_out, relate, unrelate};

/// Query to relate a playlist to its songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->playlist_to_song->$songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::playlist::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->playlist_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> RelateStatement {
    relate("id", "songs", "playlist_to_song")
}

/// Query to read the songs of a playlist
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->playlist_to_song.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::playlist::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id->playlist_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_songs() -> SelectStatement {
    read_related_out("id", "playlist_to_song")
}

/// Query to remove songs from a playlist
///
/// Compiles to:    
/// ```sql, ignore
/// DELETE $id->playlist_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::playlist::remove_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $id->playlist_to_song WHERE out IN $songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn remove_songs() -> DeleteStatement {
    unrelate("id", "songs", "playlist_to_song")
}

/// Query to "repair" a collection.
///
/// This query updates the `song_count` and runtime of the collection.
///
/// Compiles to:
/// ```sql, ignore
/// UPDATE $id SET song_count=$songs, runtime=$runtime
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::playlist::repair;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = repair();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "UPDATE $id SET song_count=$songs, runtime=$runtime".into_query().unwrap()
/// );
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

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_add_songs() {
        let statement = add_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $id->playlist_to_song->$songs".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_songs() {
        let statement = read_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id->playlist_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_remove_songs() {
        let statement = remove_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $id->playlist_to_song WHERE out IN $songs"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_repair() {
        let statement = repair();
        assert_eq!(
            statement.into_query().unwrap(),
            "UPDATE $id SET song_count=$songs, runtime=$runtime"
                .into_query()
                .unwrap()
        );
    }
}
