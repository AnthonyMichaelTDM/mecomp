//! CRUD operations for the song table

use std::path::PathBuf;

#[cfg(feature = "analysis")]
use mecomp_analysis::decoder::Decoder;
use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::song::{read_album, read_album_artist, read_artist, read_song_by_path},
        schemas::{
            album::Album,
            artist::Artist,
            song::{Song, SongChangeSet, SongId, SongMetadata, TABLE_NAME},
        },
    },
    errors::{Error, SongIOError},
};
use one_or_many::OneOrMany;

impl Song {
    #[instrument]
    pub async fn create<C: Connection>(db: &Surreal<C>, song: Self) -> Result<Option<Self>, Error> {
        Ok(db
            .create((TABLE_NAME, song.id.clone()))
            .content(song)
            .await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Self>, Error> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(db: &Surreal<C>, id: SongId) -> Result<Option<Self>, Error> {
        Ok(db.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_by_path<C: Connection>(
        db: &Surreal<C>,
        path: PathBuf,
    ) -> Result<Option<Self>, Error> {
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
    ) -> Result<Option<Album>, Error> {
        Ok(db.query(read_album()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn read_artist<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> Result<OneOrMany<Artist>, Error> {
        let res: Vec<Artist> = db.query(read_artist()).bind(("id", id)).await?.take(0)?;

        Ok(res.into())
    }

    #[instrument]
    pub async fn read_album_artist<C: Connection>(
        db: &Surreal<C>,
        id: SongId,
    ) -> Result<OneOrMany<Artist>, Error> {
        let res: Vec<Artist> = db
            .query(read_album_artist())
            .bind(("id", id))
            .await?
            .take(0)?;

        Ok(res.into())
    }

    #[instrument]
    pub async fn search<C: Connection>(
        db: &Surreal<C>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Self>, Error> {
        Ok(db
            .query("SELECT *, search::score(0) * 2 + search::score(1) * 1 AS relevance FROM song WHERE title @0@ $query OR artist @1@ $query ORDER BY relevance DESC LIMIT $limit")
            .bind(("query", query))
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
    ) -> Result<Option<Self>, Error> {
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
                Album::remove_songs(db, old_album.id, &[id.clone()]).await?;
            }

            // add song to the new album
            Album::add_songs(db, new_album.id, &[id.clone()]).await?;
        }

        if let Some(artist) = &changes.artist {
            let old_artist: OneOrMany<Artist> = Self::read_artist(db, id.clone()).await?;
            // find/create artists with the new names
            let new_artist = Artist::read_or_create_by_names(db, artist.clone()).await?;

            // remove song from the old artists
            for artist in old_artist {
                Artist::remove_songs(db, artist.id, &[id.clone()]).await?;
            }
            // add song to the new artists
            for artist in new_artist {
                Artist::add_songs(db, artist.id, &[id.clone()]).await?;
            }
        }

        Ok(db.update((TABLE_NAME, id.clone())).merge(changes).await?)
    }

    /// Delete a song from the database,
    /// will also:
    /// - go through the artist and album tables and remove references to it from there.
    /// - remove the song from playlists.
    /// - remove the song from collections.
    #[instrument]
    pub async fn delete<C: Connection>(db: &Surreal<C>, id: SongId) -> Result<Option<Self>, Error> {
        Ok(db.delete((TABLE_NAME, id)).await?)
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
    ) -> Result<Self, Error> {
        // check if the file exists
        if !metadata.path_exists() {
            return Err(SongIOError::FileNotFound(metadata.path).into());
        }

        // a separate thread, start analyzing the song
        #[cfg(feature = "analysis")]
        let path = metadata.path.clone();
        #[cfg(feature = "analysis")]
        let analysis_handle =
            std::thread::spawn(move || mecomp_analysis::decoder::MecompDecoder::analyze_path(path));

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
            #[cfg(not(feature = "analysis"))]
            analysis: [0.; 20],
            #[cfg(feature = "analysis")]
            analysis: *analysis_handle.join().unwrap()?.inner(),
        };
        // add that song to the database
        let song_id = Self::create(db, song.clone()).await?.unwrap().id;

        // add the song to the artists, if it's not already there (which it won't be)
        for artist in &artists {
            Artist::add_songs(db, artist.id.clone(), &[song_id.clone()]).await?;
        }

        // add the song to the album, if it's not already there (which it won't be)
        Album::add_songs(db, album.id.clone(), &[song_id.clone()]).await?;

        Ok(song)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::{
        arb_song_case, create_song_metadata, create_song_with_overrides, init_test_database,
    };

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;

        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song").into(),
            artist: vec![format!("Test Artist").into()].into(),
            album_artist: vec![format!("Test Artist").into()].into(),
            album: format!("Test Album").into(),
            genre: OneOrMany::One(format!("Test Genre").into()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: format!("song.mp3").into(),
            analysis: [0.; 20],
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
        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
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
        Artist::add_songs(&db, artist.id.clone(), &[song.id.clone()]).await?;
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
        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
        let mut artist = Artist::read_or_create_by_names(&db, song.album_artist.clone()).await?;
        artist.sort_by(|a, b| a.id.cmp(&b.id));

        let mut read: Vec<Artist> = Vec::from(Song::read_album_artist(&db, song.id.clone()).await?);
        read.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(artist, read);
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
        let changes = SongChangeSet {
            title: Some(format!("Updated Title ").into()),
            runtime: Some(Duration::from_secs(10)),
            track: Some(Some(2)),
            disc: Some(Some(2)),
            genre: Some(OneOrMany::One("Updated Genre".into())),
            release_year: Some(Some(2021)),
            extension: Some("flac".into()),
            ..Default::default()
        };
        // test updating things that don't require relation repair
        let updated = create_song_with_overrides(&db, arb_song_case()(), changes.clone()).await?;

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
            artist: Some(OneOrMany::One(format!("Updated Artist").into())),
            ..Default::default()
        };
        // test updating the artist
        let updated = create_song_with_overrides(&db, arb_song_case()(), changes.clone()).await?;

        assert_eq!(updated.artist, changes.artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> =
            Artist::read_or_create_by_names(&db, changes.artist.unwrap())
                .await?
                .into();
        assert_eq!(
            new_artist,
            Song::read_artist(&db, updated.id.clone()).await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_update_album_artist() -> Result<()> {
        let db = init_test_database().await?;
        let changes = SongChangeSet {
            album_artist: Some(OneOrMany::One(format!("Updated Album Artist").into())),
            ..Default::default()
        };
        // test updating the album artist
        let updated = create_song_with_overrides(&db, arb_song_case()(), changes.clone()).await?;

        assert_eq!(updated.album_artist, changes.album_artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> =
            Artist::read_or_create_by_names(&db, changes.album_artist.unwrap())
                .await?
                .into();
        assert_eq!(
            new_artist,
            Song::read_album_artist(&db, updated.id.clone()).await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_update_album() -> Result<()> {
        let db = init_test_database().await?;
        let changes = SongChangeSet {
            album: Some(format!("Updated Album").into()),
            ..Default::default()
        };
        // test updating the album
        let updated = create_song_with_overrides(&db, arb_song_case()(), changes.clone()).await?;

        assert_eq!(updated.album, changes.album.clone().unwrap());

        // since the new album didn't exist before, it should have been created
        let new_album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &changes.album.unwrap(),
            updated.album_artist.clone(),
        )
        .await?;
        assert_eq!(new_album, Song::read_album(&db, updated.id.clone()).await?);
        assert!(new_album.is_some());
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
