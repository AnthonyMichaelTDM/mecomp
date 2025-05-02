use crate::db::{queries::parse_query, schemas};

use surrealdb::opt::IntoQuery;

use super::{
    generic::{read_related_out, relate, unrelate},
    relations::{ARTIST_TO_ALBUM, ARTIST_TO_SONG},
};

/// Query to read an artist by their name.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM artist WHERE name = $name LIMIT 1
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_by_name;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_name();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM artist WHERE name = $name LIMIT 1".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_name() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE name = $name LIMIT 1",
        schemas::artist::TABLE_NAME
    ))
}

/// Query to read a artists by their names.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM artist WHERE name IN $names
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_by_names;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_by_names();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM artist WHERE name IN $names".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_by_names() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE name IN $names",
        schemas::artist::TABLE_NAME
    ))
}

/// Query to read the albums by an artist.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id->artist_to_album.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_albums;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_albums();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id->artist_to_album.out".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_albums() -> impl IntoQuery {
    read_related_out("*", "id", ARTIST_TO_ALBUM)
}

/// Query to relate an artist to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_album->$album
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_album;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_album();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_album() -> impl IntoQuery {
    relate("id", "album", ARTIST_TO_ALBUM)
}

/// Query to relate artists to an album.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $ids->artist_to_album->$album
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_album_to_artists;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_album_to_artists();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $ids->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_album_to_artists() -> impl IntoQuery {
    relate("ids", "album", ARTIST_TO_ALBUM)
}

/// Query to relate an artists to songs.
///
/// Compiles to:
/// ```sql, ignore
/// RELATE $id->artist_to_song->$songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::add_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = add_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_song->$songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn add_songs() -> impl IntoQuery {
    relate("id", "songs", ARTIST_TO_SONG)
}

/// Query to remove songs from an artist.
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $artist->artist_to_song WHERE out IN $songs
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::remove_songs;    
/// use surrealdb::opt::IntoQuery;
///
/// let statement = remove_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $artist->artist_to_song WHERE out IN $songs".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn remove_songs() -> impl IntoQuery {
    unrelate("artist", "songs", ARTIST_TO_SONG)
}

/// Query to read all the songs associated with an artist.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM array::union($artist->artist_to_song.out, $artist->artist_to_album->album->album_to_song.out)
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_songs;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_songs();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM array::union($artist->artist_to_song.out, $artist->artist_to_album->album->album_to_song.out)".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_songs() -> impl IntoQuery {
    "SELECT * FROM array::union($artist->artist_to_song.out, $artist->artist_to_album->album->album_to_song.out)"
}

#[cfg(test)]
mod query_validation_tests {
    use rstest::rstest;

    use crate::db::queries::validate_query;

    use super::*;

    #[rstest]
    #[case::read_by_name(read_by_name(), "SELECT * FROM artist WHERE name = $name LIMIT 1")]
    #[case::read_by_names(read_by_names(), "SELECT * FROM artist WHERE name IN $names")]
    #[case::read_albums(read_albums(), "SELECT * FROM $id->artist_to_album.out")]
    #[case::add_album(add_album(), "RELATE $id->artist_to_album->$album")]
    #[case::add_album_to_artists(add_album_to_artists(), "RELATE $ids->artist_to_album->$album")]
    #[case::add_songs(add_songs(), "RELATE $id->artist_to_song->$songs")]
    #[case::remove_songs(remove_songs(), "DELETE $artist->artist_to_song WHERE out IN $songs")]
    #[case::read_songs(
        read_songs(),
        "SELECT * FROM array::union($artist->artist_to_song.out, $artist->artist_to_album->album->album_to_song.out)"
    )]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
