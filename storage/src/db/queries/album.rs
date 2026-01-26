use surrealdb::opt::IntoQuery;
use surrealqlx::surrql;

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
pub const fn read_by_name() -> impl IntoQuery {
    surrql!("SELECT * FROM album WHERE title=$name")
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
pub const fn read_by_name_and_album_artist() -> impl IntoQuery {
    surrql!("SELECT * FROM album WHERE title=$title AND artist=$artist")
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
/// use mecomp_storage::db::crud::queries::album::add_song;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_song();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $album->album_to_song->$song".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn add_song() -> impl IntoQuery {
    surrql!("RELATE $album->album_to_song->$song")
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
pub const fn read_songs() -> impl IntoQuery {
    surrql!("SELECT * FROM $album->album_to_song.out")
}

/// Query to remove songs from an album
///
/// Compiles to:
///
/// ```sql, ignore
/// DELETE $album->album_to_song WHERE out == $song
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::album::remove_song;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_song();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $album->album_to_song WHERE out == $song".into_query().unwrap()
/// );
#[must_use]
#[inline]
pub const fn remove_song() -> impl IntoQuery {
    surrql!("DELETE $album->album_to_song WHERE out == $song")
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
pub const fn read_artist() -> impl IntoQuery {
    surrql!("SELECT * FROM $id<-artist_to_album.in")
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
    #[case::add_song(add_song(), "RELATE $album->album_to_song->$song")]
    #[case::read_songs(read_songs(), "SELECT * FROM $album->album_to_song.out")]
    #[case::remove_song(remove_song(), "DELETE $album->album_to_song WHERE out == $song")]
    #[case::read_artist(read_artist(), "SELECT * FROM $id<-artist_to_album.in")]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
