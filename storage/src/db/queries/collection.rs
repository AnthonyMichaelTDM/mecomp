use surrealdb::opt::IntoQuery;

use super::{
    generic::{read_related_out, relate, unrelate},
    relations::COLLECTION_TO_SONG,
};

/// Query to relate a collection to its songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->collection_to_song->$songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::collection::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->collection_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> impl IntoQuery {
    relate("id", "songs", COLLECTION_TO_SONG)
}

/// Query to read the songs of a collection
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->collection_to_song.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::collection::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id->collection_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_songs() -> impl IntoQuery {
    read_related_out("id", COLLECTION_TO_SONG)
}

/// Query to remove songs from a collection
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $id->collection_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::collection::remove_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $id->collection_to_song WHERE out IN $songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn remove_songs() -> impl IntoQuery {
    unrelate("id", "songs", COLLECTION_TO_SONG)
}

#[cfg(test)]
mod query_validation_tests {
    use rstest::rstest;

    use crate::db::queries::validate_query;

    use super::*;

    #[rstest]
    #[case::add_songs(add_songs(), "RELATE $id->collection_to_song->$songs")]
    #[case::read_songs(read_songs(), "SELECT * FROM $id->collection_to_song.out")]
    #[case::remove_songs(remove_songs(), "DELETE $id->collection_to_song WHERE out IN $songs")]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
