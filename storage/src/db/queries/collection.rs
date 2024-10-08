use surrealdb::opt::IntoQuery;

use super::generic::{read_related_out, relate, unrelate};

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
    relate("id", "songs", "collection_to_song")
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
    read_related_out("id", "collection_to_song")
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
    unrelate("id", "songs", "collection_to_song")
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
            "RELATE $id->collection_to_song->$songs"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_songs() {
        let statement = read_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id->collection_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_remove_songs() {
        let statement = remove_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $id->collection_to_song WHERE out IN $songs"
                .into_query()
                .unwrap()
        );
    }
}
