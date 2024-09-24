use surrealdb::opt::IntoQuery;

use crate::db::schemas;

use super::generic::read_related_in;

/// Query to read a song by its path
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM song WHERE path = $path LIMIT 1
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_song_by_path;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_song_by_path();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM song WHERE path = $path LIMIT 1".into_query().unwrap()
/// );
/// ```
///
/// # Panics
///
/// This function will panic if the query cannot be parsed, which should never happen.
#[must_use]
pub fn read_song_by_path() -> impl IntoQuery {
    format!(
        "SELECT * FROM {} WHERE path = $path LIMIT 1",
        schemas::song::TABLE_NAME
    )
    .into_query()
    .unwrap()
}

/// query to read the album of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-album_to_song.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_album;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_album();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-album_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_album() -> impl IntoQuery {
    read_related_in("id", "album_to_song")
}

/// Query to read the artist of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-artist_to_song.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-artist_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_artist() -> impl IntoQuery {
    read_related_in("id", "artist_to_song")
}

/// Query to read the album artist of a song
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_album_artist;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_album_artist();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in".into_query().unwrap()
/// );
/// ```
///
/// # Panics
///
/// This function will panic if the query cannot be parsed, which should never happen.
#[must_use]
pub fn read_album_artist() -> impl IntoQuery {
    "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in"
        .into_query()
        .unwrap()
}

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_read_song_by_path() {
        let statement = read_song_by_path();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM song WHERE path = $path LIMIT 1"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_album() {
        let statement = read_album();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id<-album_to_song.in".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_artist() {
        let statement = read_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id<-artist_to_song.in".into_query().unwrap()
        );
    }

    #[test]
    fn test_read_album_artist() {
        let statement = read_album_artist();
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in"
                .into_query()
                .unwrap()
        );
    }
}
