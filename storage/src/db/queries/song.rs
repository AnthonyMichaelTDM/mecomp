use surrealdb::opt::IntoQuery;

use crate::db::schemas;

use super::{
    generic::read_related_in,
    parse_query,
    relations::{ALBUM_TO_SONG, ARTIST_TO_SONG, COLLECTION_TO_SONG, PLAYLIST_TO_SONG},
};

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
#[must_use]
#[inline]
pub fn read_song_by_path() -> impl IntoQuery {
    parse_query(format!(
        "SELECT * FROM {} WHERE path = $path LIMIT 1",
        schemas::song::TABLE_NAME
    ))
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
    read_related_in("id", ALBUM_TO_SONG)
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
    read_related_in("id", ARTIST_TO_SONG)
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
#[must_use]
#[inline]
pub const fn read_album_artist() -> impl IntoQuery {
    "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in"
}

/// Query to read the playlists a song is in
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $id<-playlist_to_song.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_playlists;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_playlists();
/// assert_eq!(
///    statement.into_query().unwrap(),
///   "SELECT * FROM $id<-playlist_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_playlists() -> impl IntoQuery {
    read_related_in("id", PLAYLIST_TO_SONG)
}

/// Query to read the collections a song is in
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $id<-collection_to_song.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::song::read_collections;
/// use surrealdb::opt::IntoQuery;
///
/// let statement = read_collections();
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $id<-collection_to_song.in".into_query().unwrap()
/// );
/// ```
#[must_use]
#[inline]
pub fn read_collections() -> impl IntoQuery {
    read_related_in("id", COLLECTION_TO_SONG)
}

#[cfg(test)]
mod query_validation_tests {
    use crate::db::queries::validate_query;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::read_song_by_path(read_song_by_path(), "SELECT * FROM song WHERE path = $path LIMIT 1")]
    #[case::read_album(read_album(), "SELECT * FROM $id<-album_to_song.in")]
    #[case::read_artist(read_artist(), "SELECT * FROM $id<-artist_to_song.in")]
    #[case::read_album_artist(
        read_album_artist(),
        "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in"
    )]
    #[case::read_playlists(read_playlists(), "SELECT * FROM $id<-playlist_to_song.in")]
    #[case::read_collections(read_collections(), "SELECT * FROM $id<-collection_to_song.in")]
    fn test_queries(#[case] query: impl IntoQuery, #[case] expected: &str) {
        validate_query(query, expected);
    }
}
