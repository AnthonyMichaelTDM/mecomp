//! this module hold the implementations of functions needed for the health check of the database

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use surrealqlx::traits::Table;

use crate::db::queries::generic::{count, count_orphaned, count_orphaned_both};
use crate::db::schemas::{
    album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song,
};
use crate::errors::Error;

/// Count the number of albums in the database
#[instrument]
pub async fn count_albums<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db.query(count(Album::TABLE_NAME)).await?.take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of artists in the database
#[instrument]
pub async fn count_artists<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db.query(count(Artist::TABLE_NAME)).await?.take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of playlists in the database
#[instrument]
pub async fn count_playlists<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db.query(count(Playlist::TABLE_NAME)).await?.take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of collections in the database
#[instrument]
pub async fn count_collections<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db.query(count(Collection::TABLE_NAME)).await?.take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of songs in the database
#[instrument]
pub async fn count_songs<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db.query(count(Song::TABLE_NAME)).await?.take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of songs without analysis in the database
#[cfg(feature = "analysis")]
#[instrument]
pub async fn count_unanalyzed_songs<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: usize = super::schemas::analysis::Analysis::read_songs_without_analysis(db)
        .await?
        .len();
    Ok(result)
}

/// Count the number of orphaned albums in the database
/// This is the number of albums that have no songs
#[instrument]
pub async fn count_orphaned_albums<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db
        .query(count_orphaned(Album::TABLE_NAME, "album_to_song"))
        .await?
        .take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of orphaned artists in the database
/// This is the number of artists that have no songs, and no albums
#[instrument]
pub async fn count_orphaned_artists<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db
        .query(count_orphaned_both(
            Artist::TABLE_NAME,
            "artist_to_album",
            "artist_to_song",
        ))
        .await?
        .take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of orphaned collections in the database
/// This is the number of collections that have no songs
#[instrument]
pub async fn count_orphaned_collections<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db
        .query(count_orphaned(Collection::TABLE_NAME, "collection_to_song"))
        .await?
        .take(0)?;
    Ok(result.unwrap_or_default())
}

/// Count the number of orphaned playlists in the database
/// This is the number of playlists that have no songs
#[instrument]
pub async fn count_orphaned_playlists<C: Connection>(db: &Surreal<C>) -> Result<usize, Error> {
    let result: Option<usize> = db
        .query(count_orphaned(Playlist::TABLE_NAME, "playlist_to_song"))
        .await?
        .take(0)?;
    Ok(result.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        db::schemas::song::SongChangeSet,
        test_utils::{arb_song_case, create_song_with_overrides, init_test_database},
    };

    use super::*;
    use one_or_many::OneOrMany;
    use pretty_assertions::assert_eq;

    pub fn album() -> Album {
        Album {
            id: Album::generate_id(),
            title: "Test Album".into(),
            artist: vec!["Test Artist".into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        }
    }

    pub fn artist() -> Artist {
        Artist {
            id: Artist::generate_id(),
            name: "Test Artist".into(),
            runtime: Duration::from_secs(0),
            song_count: 0,
            album_count: 0,
        }
    }

    pub fn collection() -> Collection {
        Collection {
            id: Collection::generate_id(),
            name: "Test Collection".into(),
            song_count: 0,
            runtime: Duration::from_secs(0),
        }
    }

    pub fn playlist() -> Playlist {
        Playlist {
            id: Playlist::generate_id(),
            name: "Test Playlist".into(),
            song_count: 0,
            runtime: Duration::from_secs(0),
        }
    }

    #[tokio::test]
    async fn test_album_counting() {
        let db = init_test_database().await.unwrap();

        // initially, there should be no albums
        assert_eq!(count_albums(&db).await.unwrap(), 0);
        assert_eq!(count_orphaned_albums(&db).await.unwrap(), 0);

        // if we add a new album, there will be one album, and that album will be orphaned
        let album = album();
        Album::create(&db, album.clone()).await.unwrap();
        assert_eq!(count_albums(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_albums(&db).await.unwrap(), 1);

        // if we add a new song to the album, the album will no longer be orphaned
        let song = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                album: Some(album.title.clone()),
                album_artist: Some(album.artist.clone()), // NOTE: if we don't specify the album artist, a new album will be created instead of adding the song to the existing album
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(count_albums(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_albums(&db).await.unwrap(), 0);

        // if we delete that song, the album will be orphaned again
        Song::delete(&db, (song.id, false)).await.unwrap();
        assert_eq!(count_albums(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_albums(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_artist_counting() {
        let db = init_test_database().await.unwrap();

        // initially, there should be no artists
        assert_eq!(count_artists(&db).await.unwrap(), 0);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 0);

        // if we add a new artist, there will be one artist, and that artist will be orphaned
        let artist = artist();
        Artist::create(&db, artist.clone()).await.unwrap();
        assert_eq!(count_artists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 1);

        // if we add a new album to the artist, the artist will no longer be orphaned
        let album = album();
        Album::create(&db, album.clone()).await.unwrap();
        Artist::add_album(&db, artist.id.clone(), album.id.clone())
            .await
            .unwrap();
        assert_eq!(count_artists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 0);

        // if we delete that album, the artist will be orphaned again
        Album::delete(&db, album.id).await.unwrap();
        assert_eq!(count_artists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 1);

        // if we add a new song to the artist, the artist will no longer be orphaned
        let song = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                artist: Some(OneOrMany::One(artist.name)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(count_artists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 0);

        // if we delete that song, the artist will be orphaned again
        Song::delete(&db, (song.id, false)).await.unwrap();
        assert_eq!(count_artists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_artists(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_collection_counting() {
        let db = init_test_database().await.unwrap();

        // initially, there should be no collections
        assert_eq!(count_collections(&db).await.unwrap(), 0);
        assert_eq!(count_orphaned_collections(&db).await.unwrap(), 0);

        // if we add a new collection, there will be one collection, and that collection will be orphaned
        let collection = collection();
        Collection::create(&db, collection.clone()).await.unwrap();
        assert_eq!(count_collections(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_collections(&db).await.unwrap(), 1);

        // if we add a new song to the collection, the collection will no longer be orphaned
        let song = create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
            .await
            .unwrap();
        Collection::add_songs(&db, collection.id, vec![song.id.clone()])
            .await
            .unwrap();
        assert_eq!(count_collections(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_collections(&db).await.unwrap(), 0);

        // if we delete that song, the collection will be orphaned again
        Song::delete(&db, (song.id, false)).await.unwrap();
        assert_eq!(count_collections(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_collections(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_playlist_counting() {
        let db = init_test_database().await.unwrap();

        // initially, there should be no playlists
        assert_eq!(count_playlists(&db).await.unwrap(), 0);
        assert_eq!(count_orphaned_playlists(&db).await.unwrap(), 0);

        // if we add a new playlist, there will be one playlist, and that playlist will be orphaned
        let playlist = playlist();
        Playlist::create(&db, playlist.clone()).await.unwrap();
        assert_eq!(count_playlists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_playlists(&db).await.unwrap(), 1);

        // if we add a new song to the playlist, the playlist will no longer be orphaned
        let song = create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
            .await
            .unwrap();
        Playlist::add_songs(&db, playlist.id, vec![song.id.clone()])
            .await
            .unwrap();
        assert_eq!(count_playlists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_playlists(&db).await.unwrap(), 0);

        // if we delete that song, the playlist will be orphaned again
        Song::delete(&db, song.id).await.unwrap();
        assert_eq!(count_playlists(&db).await.unwrap(), 1);
        assert_eq!(count_orphaned_playlists(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_song_counting() {
        let db = init_test_database().await.unwrap();

        // initially, there should be no songs
        assert_eq!(count_songs(&db).await.unwrap(), 0);

        // if we add a new song, there will be one song
        let song = create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default())
            .await
            .unwrap();
        assert_eq!(count_songs(&db).await.unwrap(), 1);

        // if we delete that song, there will be no songs
        Song::delete(&db, song.id).await.unwrap();
        assert_eq!(count_songs(&db).await.unwrap(), 0);
    }
}
