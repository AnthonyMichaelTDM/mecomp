use crate::db::{queries::parse_query, schemas};
use surrealdb::opt::IntoQuery;

use super::{
    generic::{read_related_in, read_related_out, relate, unrelate},
    relations::{ALBUM_TO_SONG, ARTIST_TO_ALBUM},
};

/// Query to read an album by its name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM album WHERE title=$name
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_by_name;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM album WHERE title=$name".into_query().unwrap()
/// );
/// ```
#[allow(clippy::missing_panics_doc)] // can only panic if the query is invalid, which should never happen
#[must_use]
pub fn read_by_name() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE title=$name",
        schemas::album::TABLE_NAME
    ))
}

/// Query to read an album by its name and album artist.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM album WHERE title=$title AND artist=$artist
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_by_name_and_album_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name_and_album_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM album WHERE title=$title AND artist=$artist".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_name_and_album_artist() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE title=$title AND artist=$artist",
        schemas::album::TABLE_NAME
    ))
}

/// Query to relate an album to its songs.
///
/// Compiles to:
///
/// ```sql, ignore
/// RELATE $album->album_to_song->$songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $album->album_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> impl IntoQuery {
    relate("album", "songs", ALBUM_TO_SONG)
}

/// Query to read the songs of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $album->album_to_song.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $album->album_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_songs() -> impl IntoQuery {
    read_related_out("album", ALBUM_TO_SONG)
}

/// Query to remove songs from an album
///
/// Compiles to:
///
/// ```sql, ignore
/// DELETE $album->album_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::remove_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $album->album_to_song WHERE out IN $songs".into_query().unwrap()
/// );
#[must_use]
#[inline]
pub fn remove_songs() -> impl IntoQuery {
    unrelate("album", "songs", ALBUM_TO_SONG)
}

/// Query to read the artist of an album
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $id<-artist_to_album.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::read_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-artist_to_album.in".into_query().unwrap()
/// );
#[must_use]
#[inline]
pub fn read_artist() -> impl IntoQuery {
    read_related_in("id", ARTIST_TO_ALBUM)
}

#[cfg(test)]
mod query_validation_tests {
    use rstest::rstest;

    use super::*;
    use crate::db::queries::validate_query;

    #[rstest]
    #[case::read_by_name(read_by_name(), "SELECT * FROM album WHERE title=$name")]
    #[case::read_by_name_and_album_artist(
        read_by_name_and_album_artist(),
        "SELECT * FROM album WHERE title=$title AND artist=$artist"
    )]
    #[case::add_songs(add_songs(), "RELATE $album->album_to_song->$songs")]
    #[case::read_songs(read_songs(), "SELECT * FROM $album->album_to_song.out")]
    #[case::remove_songs(remove_songs(), "DELETE $album->album_to_song WHERE out IN $songs")]
    #[case::read_artist(read_artist(), "SELECT * FROM $id<-artist_to_album.in")]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
