use surrealdb::opt::IntoQuery;

use crate::db::schemas;

use super::{
    generic::{read_related_out, relate, unrelate},
    parse_query,
};

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
pub fn add_songs() -> impl IntoQuery {
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
pub fn read_songs() -> impl IntoQuery {
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
pub fn remove_songs() -> impl IntoQuery {
    unrelate("id", "songs", "playlist_to_song")
}

/// Query to read a playlist by its name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM playlist WHERE name = $name LIMIT 1
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::playlist::read_by_name;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM playlist WHERE name = $name LIMIT 1".into_query().unwrap()
/// );
/// ```
///
/// # Panics
///
/// This function will panic if the query cannot be parsed, which should never happen.
#[must_use]
#[inline]
pub fn read_by_name() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE name = $name LIMIT 1",
        schemas::playlist::TABLE_NAME
    ))
}

#[cfg(test)]
mod query_validation_tests {
    use crate::db::queries::validate_query;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::add_songs(add_songs(), "RELATE $id->playlist_to_song->$songs")]
    #[case::read_songs(read_songs(), "SELECT * FROM $id->playlist_to_song.out")]
    #[case::remove_songs(remove_songs(), "DELETE $id->playlist_to_song WHERE out IN $songs")]
    #[case::read_by_name(read_by_name(), "SELECT * FROM playlist WHERE name = $name LIMIT 1")]
    fn test_queries(#[case] statement: impl IntoQuery, #[case] expected: &str) {
        validate_query(statement, expected);
    }
}
