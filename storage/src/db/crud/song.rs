//! CRUD operations for the song table

use std::path::PathBuf;

use log::info;
use surrealdb::{Connection, RecordId, Surreal};
use tracing::instrument;

#[cfg(feature = "analysis")]
use crate::db::schemas::analysis::Analysis;
use crate::{
    db::{
        queries::song::{
            read_album, read_album_artist, read_artist, read_collections, read_playlists,
            read_song_by_path,
        },
        schemas::{
            album::Album,
            artist::Artist,
            collection::Collection,
            playlist::Playlist,
            song::{Song, SongChangeSet, SongId, SongMetadata, TABLE_NAME},
        },
    },
    errors::{Error, SongIOError, StorageResult},
};
use one_or_many::OneOrMany;

#[derive(Debug)]
struct DeleteArgs {
    pub id: SongId,
    pub delete_orphans: bool,
}

impl From<SongId> for DeleteArgs {
    fn from(id: SongId) -> Self {
        Self {
            id,
            delete_orphans: true,
        }
    }
}

impl From<(SongId, bool)> for DeleteArgs {
    fn from(tuple: (SongId, bool)) -> Self {
        Self {
            id: tuple.0,
            delete_orphans: tuple.1,
        }
    }
}

impl Song {
    #[instrument]
    pub async fn create<C: Connection>(db: &Surreal<C>, song: Self) -> StorageResult<Option<Self>> {
        Ok(db
            .create(RecordId::from_inner(song.id.clone()))
            .content(song)
            .await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(db: &Surreal<C>, id: SongId) -> StorageResult<Option<Self>> {
        Ok(db.select(RecordId::from_inner(id)).await?)
    }

    #[instrument]
    pub async fn read_by_path<C: Connection>(
        db: &Surreal<C>,
        path: PathBuf,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .query(read_song_by_path())
            .bind(("path", path))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_album<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> StorageResult<Option<Album>> {
        Ok(db.query(read_album()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn read_artist<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> StorageResult<OneOrMany<Artist>> {
        Ok(db.query(read_artist()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn read_album_artist<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> StorageResult<OneOrMany<Artist>> {
        Ok(db
            .query(read_album_artist())
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_playlists<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> StorageResult<Vec<Playlist>> {
        Ok(db.query(read_playlists()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn read_collections<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> StorageResult<Vec<Collection>> {
        Ok(db
            .query(read_collections())
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn search<C: Connection>(
        db: &Surreal<C>,
        query: &str,
        limit: i64,
    ) -> StorageResult<Vec<Self>> {
        Ok(db
            .query("SELECT *, search::score(0) * 2 + search::score(1) * 1 AS relevance FROM song WHERE title @0@ $query OR artist @1@ $query ORDER BY relevance DESC LIMIT $limit")
            .bind(("query", query.to_owned()))
            .bind(("limit", limit))
            .await?
            .take(0)?)
    }

    /// Update the information about a song, repairs relations if necessary
    ///
    /// repairs relations if:
    /// - the artist name(s) have changed
    /// - the album name has changed
    /// - the album artist name(s) have changed
    /// - TODO: The duration has changed
    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
        changes: SongChangeSet,
    ) -> StorageResult<Option<Self>> {
        if changes.album.is_some() || changes.album_artist.is_some() {
            let old_album = Self::read_album(db, id.clone()).await?;

            // get the old album title and artist
            // priority: old album (read from db) > old album info (read from song) > unknown
            let (old_album_title, old_album_artist) = if let Some(album) = &old_album {
                (album.title.clone(), album.artist.clone())
            } else if let Some(song) = Self::read(db, id.clone()).await? {
                (song.album.clone(), song.album_artist)
            } else {
                (
                    "Unknown Album".into(),
                    OneOrMany::One("Unknown Artist".into()),
                )
            };

            // find/create the new album
            let new_album = Album::read_or_create_by_name_and_album_artist(
                db,
                &changes.album.clone().unwrap_or(old_album_title),
                changes.album_artist.clone().unwrap_or(old_album_artist),
            )
            .await?
            .ok_or(Error::NotFound)?;

            // remove song from the old album, if it existed
            if let Some(old_album) = old_album {
                if Album::remove_songs(db, old_album.id.clone(), vec![id.clone()]).await? {
                    // if the album is left without any songs, delete it
                    info!("Deleting orphaned album: {:?}", old_album.id);
                    Album::delete(db, old_album.id).await?;
                }
            }

            // remove the album from the old album artist(s)
            for artist in Self::read_album_artist(db, id.clone()).await? {
                if Artist::remove_songs(db, artist.id.clone(), vec![id.clone()]).await? {
                    // if the artist is left without any songs, delete it
                    info!("Deleting orphaned artist: {:?}", artist.id);
                    Artist::delete(db, artist.id).await?;
                }
            }

            // add song to the new album
            Album::add_songs(db, new_album.id, vec![id.clone()]).await?;
        }

        if let Some(artist) = &changes.artist {
            let old_artist: OneOrMany<Artist> = Self::read_artist(db, id.clone()).await?;
            // find/create artists with the new names
            let new_artist = Artist::read_or_create_by_names(db, artist.clone()).await?;

            // remove song from the old artists
            for artist in old_artist {
                if Artist::remove_songs(db, artist.id.clone(), vec![id.clone()]).await? {
                    // if the artist is left without any songs, delete it
                    info!("Deleting orphaned artist: {:?}", artist.id);
                    Artist::delete(db, artist.id).await?;
                }
            }
            // add song to the new artists
            for artist in new_artist {
                Artist::add_songs(db, artist.id, vec![id.clone()]).await?;
            }
        }

        Ok(db.update(RecordId::from_inner(id)).merge(changes).await?)
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there
    ///   - if the artist or album would be left without any songs, they will be deleted as well
    /// - remove the song from playlists.
    /// - remove the song from collections.
    #[instrument]
    pub async fn delete<C: Connection, Args: Into<DeleteArgs> + std::fmt::Debug + Send>(
        db: &Surreal<C>,
        args: Args,
    ) -> StorageResult<Option<Self>> {
        let args = args.into();
        let DeleteArgs { id, delete_orphans } = args;

        // delete the analysis for the song (if it exists)
        #[cfg(feature = "analysis")]
        if let Ok(Some(analysis)) = Analysis::read_for_song(db, id.clone()).await {
            Analysis::delete(db, analysis.id).await?;
        }

        // if we're not deleting orphans, we can just delete the song
        if !delete_orphans {
            return Ok(db.delete(RecordId::from_inner(id)).await?);
        }

        // remove the song from any playlists or collections it's in
        for playlist in Self::read_playlists(db, id.clone()).await? {
            Playlist::remove_songs(db, playlist.id, vec![id.clone()]).await?;
        }
        for collection in Self::read_collections(db, id.clone()).await? {
            if Collection::remove_songs(db, collection.id.clone(), vec![id.clone()]).await? {
                info!("Deleting orphaned collection: {:?}", collection.id);
                Collection::delete(db, collection.id).await?;
            }
        }
        if let Some(album) = Self::read_album(db, id.clone()).await? {
            if Album::remove_songs(db, album.id.clone(), vec![id.clone()]).await? {
                info!("Deleting orphaned album: {:?}", album.id);
                Album::delete(db, album.id).await?;
            }
        }
        for artist in Self::read_album_artist(db, id.clone()).await? {
            if Artist::remove_songs(db, artist.id.clone(), vec![id.clone()]).await? {
                info!("Deleting orphaned artist: {:?}", artist.id);
                Artist::delete(db, artist.id).await?;
            }
        }
        for artist in Self::read_artist(db, id.clone()).await? {
            if Artist::remove_songs(db, artist.id.clone(), vec![id.clone()]).await? {
                info!("Deleting orphaned artist: {:?}", artist.id);
                Artist::delete(db, artist.id).await?;
            }
        }

        Ok(db.delete(RecordId::from_inner(id)).await?)
    }

    /// Create a new [`Song`] from song metadata and load it into the database.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata of the song.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file does not exist, or if the file is not a valid audio file.
    ///
    /// # Side Effects
    ///
    /// This function will create a new [`Song`], [`Artist`], and [`Album`] if they do not exist in the database.
    /// This function will also add the new [`Song`] to the [`Artist`] and the [`Album`].
    /// This function will also update the [`Artist`] and the [`Album`] in the database.
    #[instrument]
    pub async fn try_load_into_db<C: Connection>(
        db: &Surreal<C>,
        metadata: SongMetadata,
    ) -> StorageResult<Self> {
        // check if the file exists
        if !metadata.path_exists() {
            return Err(SongIOError::FileNotFound(metadata.path).into());
        }

        // for each artist, check if the artist exists in the database and get the id, if they don't then create a new artist and get the id
        let artists = Artist::read_or_create_by_names(db, metadata.artist.clone()).await?;

        // check if the album artist exists, if they don't then create a new artist and get the id
        Artist::read_or_create_by_names(db, metadata.album_artist.clone()).await?;

        // read or create the album
        // if an album doesn't exist with the given title and album artists,
        // will create a new album with the given title and album artists
        let album = Album::read_or_create_by_name_and_album_artist(
            db,
            &metadata.album,
            metadata.album_artist.clone(),
        )
        .await?
        .ok_or(Error::NotCreated)?;

        // create a new song
        let song = Self {
            id: Self::generate_id(),
            title: metadata.title,
            artist: metadata.artist,
            album_artist: metadata.album_artist,
            album: metadata.album,
            genre: metadata.genre,
            release_year: metadata.release_year,
            runtime: metadata.runtime,
            extension: metadata.extension,
            track: metadata.track,
            disc: metadata.disc,
            path: metadata.path,
        };
        // add that song to the database
        let song_id = Self::create(db, song.clone()).await?.unwrap().id;

        // add the song to the artists, if it's not already there (which it won't be)
        for artist in &artists {
            Artist::add_songs(db, artist.id.clone(), vec![song_id.clone()]).await?;
        }

        // add the song to the album, if it's not already there (which it won't be)
        Album::add_songs(db, album.id.clone(), vec![song_id.clone()]).await?;

        Ok(song)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        db::health::{count_albums, count_artists, count_songs},
        test_utils::{
            arb_song_case, create_song_metadata, create_song_with_overrides, init_test_database,
        },
    };

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;

        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".to_string(),
            artist: vec!["Test Artist".to_string()].into(),
            album_artist: vec!["Test Artist".to_string()].into(),
            album: "Test Album".to_string(),
            genre: OneOrMany::One("Test Genre".to_string()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".to_string().into(),
        };

        let created = Song::create(&db, song.clone()).await?;
        assert_eq!(created, Some(song));
        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let _ =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let _ =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let _ =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let songs = Song::read_all(&db).await?;
        assert!(!songs.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let read = Song::read(&db, song.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(read, song);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_path() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let read = Song::read_by_path(&db, song.path.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(read, song);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_album() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let album =
            Album::read_or_create_by_name_and_album_artist(&db, &song.album, song.album_artist)
                .await?
                .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;
        let album = Album::read(&db, album.id)
            .await?
            .ok_or_else(|| anyhow!("Album not found"))?;
        assert_eq!(Some(album), Song::read_album(&db, song.id.clone()).await?);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_artist() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let artist = Artist::read_or_create_by_name(&db, song.artist.clone().first().unwrap())
            .await?
            .ok_or_else(|| anyhow!("Artist not found/created"))?;
        Artist::add_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?;
        let artist = Artist::read(&db, artist.id)
            .await?
            .ok_or_else(|| anyhow!("Artist not found"))?;
        assert_eq!(
            OneOrMany::One(artist),
            Song::read_artist(&db, song.id.clone()).await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_read_album_artist() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &song.album,
            song.album_artist.clone(),
        )
        .await?
        .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;
        let mut artist = Artist::read_or_create_by_names(&db, song.album_artist.clone()).await?;
        artist.sort_by(|a, b| a.id.cmp(&b.id));

        let mut read: Vec<Artist> = Vec::from(Song::read_album_artist(&db, song.id.clone()).await?);
        read.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(artist, read);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_playlists() -> Result<()> {
        let db = init_test_database().await?;
        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song3 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let playlist1 = Playlist::create(
            &db,
            Playlist {
                id: Playlist::generate_id(),
                name: "Test Playlist 1".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
        )
        .await?
        .unwrap();
        let playlist2 = Playlist::create(
            &db,
            Playlist {
                id: Playlist::generate_id(),
                name: "Test Playlist 2".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
        )
        .await?
        .unwrap();

        // add songs to the playlists
        Playlist::add_songs(
            &db,
            playlist1.id.clone(),
            vec![song1.id.clone(), song2.id.clone()],
        )
        .await?;
        Playlist::add_songs(
            &db,
            playlist2.id.clone(),
            vec![song2.id.clone(), song3.id.clone()],
        )
        .await?;

        let playlists = Song::read_playlists(&db, song1.id.clone()).await?;
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, playlist1.id);

        let playlists: Vec<_> = Song::read_playlists(&db, song2.id.clone())
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(playlists.len(), 2);
        assert!(playlists.contains(&playlist1.id));
        assert!(playlists.contains(&playlist2.id));

        let playlists = Song::read_playlists(&db, song3.id.clone()).await?;
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, playlist2.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_collections() -> Result<()> {
        let db = init_test_database().await?;
        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let collection1 = Collection::create(
            &db,
            Collection {
                id: Collection::generate_id(),
                name: "Test Collection 1".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
        )
        .await?
        .unwrap();
        let collection2 = Collection::create(
            &db,
            Collection {
                id: Collection::generate_id(),
                name: "Test Collection 2".into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
            },
        )
        .await?
        .unwrap();

        // add songs to the collections
        Collection::add_songs(&db, collection1.id.clone(), vec![song1.id.clone()]).await?;
        Collection::add_songs(&db, collection2.id.clone(), vec![song2.id.clone()]).await?;

        let collections = Song::read_collections(&db, song1.id.clone()).await?;
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, collection1.id);

        let collections = Song::read_collections(&db, song2.id.clone()).await?;
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, collection2.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_search_by_title() -> Result<()> {
        let db = init_test_database().await?;
        let song1 = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                title: Some("Foo Bar".into()),
                ..Default::default()
            },
        )
        .await?;
        let song2 = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                title: Some("Foo".into()),
                ..Default::default()
            },
        )
        .await?;

        let found = Song::search(&db, "Foo", 2).await?;
        assert_eq!(found.len(), 2);
        assert!(found.contains(&song1));
        assert!(found.contains(&song2));

        let found = Song::search(&db, "Bar", 10).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found, vec![song1]);

        Ok(())
    }

    #[tokio::test]
    async fn test_search_by_artist() -> Result<()> {
        let db = init_test_database().await?;
        let song1 = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                artist: Some(OneOrMany::One("Green Day".into())),
                ..Default::default()
            },
        )
        .await?;
        let song2 = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                artist: Some(OneOrMany::One("Green Day".into())),
                ..Default::default()
            },
        )
        .await?;
        let song3 = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                title: Some("green".into()),
                ..Default::default()
            },
        )
        .await?;

        let found = Song::search(&db, "Green", 3).await?;
        // assert that all 3 songs were found, and that the first one is the one with "green" in the title (since title is weighted higher than artist in the search query)
        assert_eq!(found.len(), 3);
        // assert_eq!(found, vec![]);
        assert!(found.contains(&song1));
        assert!(found.contains(&song2));
        assert!(found.contains(&song3));

        assert_eq!(found.first(), Some(&song3));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_no_repair() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let changes = SongChangeSet {
            title: Some("Updated Title ".to_string()),
            runtime: Some(Duration::from_secs(10)),
            track: Some(Some(2)),
            disc: Some(Some(2)),
            genre: Some(OneOrMany::One("Updated Genre".into())),
            release_year: Some(Some(2021)),
            extension: Some("flac".into()),
            ..Default::default()
        };
        // test updating things that don't require relation repair
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .unwrap();

        assert_eq!(updated.title, changes.title.unwrap());
        assert_eq!(updated.runtime, changes.runtime.unwrap());
        assert_eq!(updated.track, changes.track.unwrap());
        assert_eq!(updated.disc, changes.disc.unwrap());
        assert_eq!(updated.genre, changes.genre.unwrap());
        assert_eq!(updated.release_year, changes.release_year.unwrap());
        assert_eq!(updated.extension, changes.extension.unwrap());
        Ok(())
    }

    #[tokio::test]
    async fn test_update_artist() -> Result<()> {
        let db = init_test_database().await?;
        let changes = SongChangeSet {
            artist: Some(OneOrMany::One("Artist".to_string())),
            ..Default::default()
        };
        let song_case = arb_song_case()();
        let song = create_song_with_overrides(&db, song_case.clone(), changes.clone()).await?;
        // test updating the artist
        let changes = SongChangeSet {
            artist: Some(OneOrMany::One("Updated Artist".to_string())),
            ..Default::default()
        };
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .unwrap();

        assert_eq!(updated.artist, changes.artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> = Artist::read_by_names(&db, changes.artist.unwrap().into())
            .await?
            .into();
        assert_eq!(
            new_artist,
            Song::read_artist(&db, updated.id.clone()).await?
        );

        // the new artist should be the only artist in the database
        let artists = Artist::read_all(&db).await?;
        assert_eq!(artists.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_update_album_artist() -> Result<()> {
        let db = init_test_database().await?;
        let changes = SongChangeSet {
            artist: Some(OneOrMany::One("Album Artist".to_string())),
            album_artist: Some(OneOrMany::One("Album Artist".to_string())),
            ..Default::default()
        };
        let song_case = arb_song_case()();
        let song = create_song_with_overrides(&db, song_case.clone(), changes.clone()).await?;
        // test updating the album artist
        let changes = SongChangeSet {
            artist: Some(OneOrMany::One("Updated Album Artist".to_string())),
            album_artist: Some(OneOrMany::One("Updated Album Artist".to_string())),
            ..Default::default()
        };
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .unwrap();

        assert_eq!(updated.album_artist, changes.album_artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> =
            Artist::read_by_names(&db, changes.album_artist.unwrap().into())
                .await?
                .into();
        assert_eq!(
            new_artist,
            Song::read_album_artist(&db, updated.id.clone()).await?
        );

        // the new artist should be the only artist in the database
        let artists = Artist::read_all(&db).await?;
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].name, "Updated Album Artist");
        Ok(())
    }

    #[tokio::test]
    async fn test_update_album() -> Result<()> {
        let db = init_test_database().await?;
        let changes = SongChangeSet {
            album: Some("Updated Album".to_string()),
            ..Default::default()
        };
        // test updating the album
        let updated = create_song_with_overrides(&db, arb_song_case()(), changes.clone()).await?;

        assert_eq!(updated.album, changes.album.clone().unwrap());

        // since the new album didn't exist before, it should have been created
        let new_album = Album::read_by_name_and_album_artist(
            &db,
            &changes.album.unwrap(),
            updated.album_artist.clone(),
        )
        .await?;
        assert_eq!(new_album, Song::read_album(&db, updated.id.clone()).await?);
        assert!(new_album.is_some());

        // the new album should be the only album in the database
        let albums = Album::read_all(&db).await?;
        assert_eq!(albums.len(), 1);

        // the new album should be associated with the song and the album artist
        let album = new_album.unwrap();
        let album_songs = Album::read_songs(&db, album.id.clone()).await?;
        assert_eq!(album_songs.len(), 1);
        assert_eq!(album_songs[0].id, updated.id);

        let album_artists = Song::read_album_artist(&db, updated.id.clone()).await?;
        let album_artists = album_artists[0].clone();
        let album_artists = Artist::read_albums(&db, album_artists.id.clone()).await?;
        assert_eq!(album_artists.len(), 1);
        assert_eq!(album_artists[0].id, album.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_with_orphan_pruning() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let deleted = Song::delete(&db, (song.id.clone(), true)).await?;
        assert_eq!(deleted, Some(song.clone()));

        let read = Song::read(&db, song.id.clone()).await?;
        assert_eq!(read, None);

        // database should be empty
        assert_eq!(count_songs(&db).await?, 0);
        assert_eq!(count_artists(&db).await?, 0);
        assert_eq!(count_albums(&db).await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_without_orphan_pruning() -> Result<()> {
        let db = init_test_database().await?;
        let song_case = arb_song_case()();
        let song =
            create_song_with_overrides(&db, song_case.clone(), SongChangeSet::default()).await?;
        let album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &song.album,
            song.album_artist.clone(),
        )
        .await?
        .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;
        let artists = Artist::read_or_create_by_names(&db, song.artist.clone()).await?;
        assert!(!artists.is_empty());
        for artist in artists {
            Artist::add_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?;
        }
        let album_artists = Artist::read_or_create_by_names(&db, song.album_artist.clone()).await?;
        assert!(!album_artists.is_empty());
        for artist in album_artists {
            Artist::add_album(&db, artist.id.clone(), album.id.clone()).await?;
        }

        let deleted = Song::delete(&db, (song.id.clone(), false)).await?;
        assert_eq!(deleted, Some(song.clone()));

        let read = Song::read(&db, song.id.clone()).await?;
        assert_eq!(read, None);

        // database should be empty
        assert_eq!(count_songs(&db).await?, 0);
        assert_eq!(
            count_artists(&db).await?,
            song_case
                .album_artists
                .iter()
                .chain(song_case.artists.iter())
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
        assert_eq!(count_albums(&db).await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_with_orphaned_album() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &song.album,
            song.album_artist.clone(),
        )
        .await?
        .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;

        let deleted = Song::delete(&db, song.id.clone()).await?;
        assert_eq!(deleted, Some(song.clone()));

        let read = Song::read(&db, song.id.clone()).await?;
        assert_eq!(read, None);

        let album = Album::read(&db, album.id.clone()).await?;
        assert_eq!(album, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_with_orphaned_artist() -> Result<()> {
        let db = init_test_database().await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let artist = Artist::read_or_create_by_name(&db, song.artist.clone().first().unwrap())
            .await?
            .ok_or_else(|| anyhow!("Artist not found/created"))?;
        Artist::add_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?;

        let deleted = Song::delete(&db, song.id.clone()).await?;
        assert_eq!(deleted, Some(song.clone()));

        let read = Song::read(&db, song.id.clone()).await?;
        assert_eq!(read, None);

        let artist = Artist::read(&db, artist.id.clone()).await?;
        assert_eq!(artist, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_try_load_into_db() {
        let db = init_test_database().await.unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        // Create a mock SongMetadata object for testing
        let metadata = create_song_metadata(&temp_dir, arb_song_case()()).unwrap();

        // Call the try_load_into_db function
        let result = Song::try_load_into_db(&db, metadata.clone()).await;

        // Assert that the function returns a valid Song object
        if let Err(e) = result {
            panic!("Error: {e:?}");
        }
        let song = result.unwrap();

        // Assert that the song has been loaded into the database correctly
        assert_eq!(song.title, metadata.title);
        assert_eq!(song.artist.len(), metadata.artist.len());
        assert_eq!(song.album_artist.len(), metadata.album_artist.len());
        assert_eq!(song.album, metadata.album);
        assert_eq!(song.genre.len(), metadata.genre.len());
        assert_eq!(song.runtime, metadata.runtime);
        assert_eq!(song.track, metadata.track);
        assert_eq!(song.disc, metadata.disc);
        assert_eq!(song.release_year, metadata.release_year);
        assert_eq!(song.extension, metadata.extension);
        assert_eq!(song.path, metadata.path);

        // Assert that the artists and album have been created in the database
        let artists = Song::read_artist(&db, song.id.clone()).await.unwrap();
        assert_eq!(artists.len(), metadata.artist.len()); // 2 artists + 1 album artist

        let album = Song::read_album(&db, song.id.clone()).await;
        assert_eq!(album.is_ok(), true);
        let album = album.unwrap();
        assert_eq!(album.is_some(), true);
        let album = album.unwrap();

        // Assert that the song has been associated with the artists and album correctly
        let artist_songs = Artist::read_songs(&db, artists.get(0).unwrap().id.clone())
            .await
            .unwrap();
        assert_eq!(artist_songs.len(), 1);
        assert_eq!(artist_songs[0].id, song.id);

        let album_songs = Album::read_songs(&db, album.id.clone()).await.unwrap();
        assert_eq!(album_songs.len(), 1);
        assert_eq!(album_songs[0].id, song.id);
    }
}
