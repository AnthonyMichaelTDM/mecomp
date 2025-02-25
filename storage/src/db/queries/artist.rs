use crate::db::schemas;

use surrealdb::opt::IntoQuery;

use super::generic::{read_related_out, relate, unrelate};

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
    format!(
        "SELECT * FROM {} WHERE name = $name LIMIT 1",
        schemas::artist::TABLE_NAME
    )
    .into_query()
    .unwrap()
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
    format!(
        "SELECT * FROM {} WHERE name IN $names",
        schemas::artist::TABLE_NAME
    )
    .into_query()
    .unwrap()
}

/// Query to read many artists
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $ids
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::artist::read_many;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_many();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $ids".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_many() -> impl IntoQuery {
    "SELECT * FROM $ids"
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
    read_related_out("id", "artist_to_album")
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
    relate("id", "album", "artist_to_album")
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
    relate("ids", "album", "artist_to_album")
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
    relate("id", "songs", "artist_to_song")
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
    unrelate("artist", "songs", "artist_to_song")
}

/// Query to read all the songs associated with an artist.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))
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
///     "RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub const fn read_songs() -> impl IntoQuery {
    "RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))"
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
            "SELECT * FROM artist WHERE name = $name LIMIT 1"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_by_names() {
        let statement = read_by_names();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM artist WHERE name IN $names"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_many() {
        let statement = read_many();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $ids".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_albums() {
        let statement = read_albums();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id->artist_to_album.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_add_album() {
        let statement = add_album();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $id->artist_to_album->$album".into_query().unwrap()
        );
    }

    #[test]
    fn test_add_album_to_artists() {
        let statement = add_album_to_artists();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $ids->artist_to_album->$album".into_query().unwrap()
        );
    }

    #[test]
    fn test_add_songs() {
        let statement = add_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $id->artist_to_song->$songs".into_query().unwrap()
        );
    }

    #[test]
    fn test_remove_songs() {
        let statement = remove_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $artist->artist_to_song WHERE out IN $songs"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_songs() {
        let statement = read_songs();
        assert_eq!(
            statement.into_query().unwrap(),
            "RETURN array::union((SELECT * FROM $artist->artist_to_song.out), (SELECT * FROM $artist->artist_to_album->album->album_to_song.out))"
                .into_query()
                .unwrap()
        );
    }
}
