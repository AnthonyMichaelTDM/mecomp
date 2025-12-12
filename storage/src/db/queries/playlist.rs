use surrealdb::opt::IntoQuery;
use surrealqlx::surrql;

/// Query to relate a playlist to its songs.
///
/// Compiles to:
/// ```sql, ignore
/// "RELATE $id->playlist_to_song->(array::complement(array::distinct($songs), $id->playlist_to_song.out))"
/// ```
///
/// Let's break this down:
/// - `RELATE $id->playlist_to_song->(...)` creates a relation between the playlist and a set of songs.
/// - `array::complement(array::distinct($songs), $id->playlist_to_song.out)` ensures that only songs
///   that are not already related to the playlist are added.
/// - `array::distinct($songs)` ensures that the input songs are unique.
/// - `$id->playlist_to_song.out` retrieves the current related songs.
#[must_use]
#[inline]
pub const fn add_songs() -> impl IntoQuery {
    // only songs that aren't already related to the playlist should be added
    surrql!(
        "RELATE $id->playlist_to_song->(array::complement(array::distinct($songs), $id->playlist_to_song.out))"
    )
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
pub const fn read_songs() -> impl IntoQuery {
    surrql!("SELECT * FROM $id->playlist_to_song.out")
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
pub const fn remove_songs() -> impl IntoQuery {
    surrql!("DELETE $id->playlist_to_song WHERE out IN $songs")
}

/// Query to read a playlist by its name
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
#[must_use]
#[inline]
pub const fn read_by_name() -> impl IntoQuery {
    surrql!("SELECT * FROM playlist WHERE name = $name LIMIT 1")
}

#[cfg(test)]
mod query_validation_tests {
    use crate::db::queries::validate_query;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::add_songs(
        add_songs(),
        "RELATE $id->playlist_to_song->(array::complement(array::distinct($songs), $id->playlist_to_song.out))"
    )]
    #[case::read_songs(read_songs(), "SELECT * FROM $id->playlist_to_song.out")]
    #[case::remove_songs(remove_songs(), "DELETE $id->playlist_to_song WHERE out IN $songs")]
    #[case::read_by_name(read_by_name(), "SELECT * FROM playlist WHERE name = $name LIMIT 1")]
    fn test_queries(#[case] statement: impl IntoQuery, #[case] expected: &str) {
        validate_query(statement, expected);
    }
}
