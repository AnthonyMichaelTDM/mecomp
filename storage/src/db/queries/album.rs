use crate::db::schemas;
use surrealdb::opt::IntoQuery;

use super::generic::{read_related_in, read_related_out, relate, unrelate};

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
#[must_use]
pub fn read_by_name() -> impl IntoQuery {
    format!(
        "SELECT * FROM {} WHERE title=$name",
        schemas::album::TABLE_NAME
    )
    .into_query()
    .unwrap()
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
    format!(
        "SELECT * FROM {} WHERE title=$title AND artist=$artist",
        schemas::album::TABLE_NAME
    )
    .into_query()
    .unwrap()
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
    relate("album", "songs", "album_to_song")
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
    read_related_out("album", "album_to_song")
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
    unrelate("album", "songs", "album_to_song")
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
    read_related_in("id", "artist_to_album")
}

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_read_by_name() {
        let statement = read_by_name();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM album WHERE title=$name"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_by_name_and_album_artist() {
        let statement = read_by_name_and_album_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM album WHERE title=$title AND artist=$artist"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_add_songs() {
        let statement = add_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $album->album_to_song->$songs".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_songs() {
        let statement = read_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $album->album_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_remove_songs() {
        let statement = remove_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $album->album_to_song WHERE out IN $songs"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_artist() {
        let statement = read_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id<-artist_to_album.in"
                .into_query()
                .unwrap()
        );
    }
}
