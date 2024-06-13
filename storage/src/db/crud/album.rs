//! CRUD operations for the album table
use std::{sync::Arc, time::Duration};

use log::warn;
use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::{
            album::{
                add_songs, read_artist, read_by_name, read_by_name_and_album_artist, read_songs,
                remove_songs,
            },
            generic::full_text_search,
        },
        schemas::{
            album::{Album, AlbumChangeSet, AlbumId, TABLE_NAME},
            artist::Artist,
            song::{Song, SongId},
        },
    },
    errors::Error,
};
use one_or_many::OneOrMany;

impl Album {
    #[instrument()]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        album: Self,
    ) -> Result<Option<Self>, Error> {
        Ok(db
            .create((TABLE_NAME, album.id.clone()))
            .content(album)
            .await?)
    }

    #[instrument()]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Self>, Error> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument()]
    pub async fn read<C: Connection>(db: &Surreal<C>, id: AlbumId) -> Result<Option<Self>, Error> {
        Ok(db.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> Result<Option<Self>, Error> {
        Ok(db.delete((TABLE_NAME, id)).await?)
    }

    #[instrument()]
    pub async fn read_by_name<C: Connection>(
        db: &Surreal<C>,
        name: &str,
    ) -> Result<Vec<Self>, Error> {
        Ok(db
            .query(read_by_name())
            .bind(("name", name))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn search<C: Connection>(
        db: &Surreal<C>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Self>, Error> {
        Ok(db
            .query(full_text_search(TABLE_NAME, "title", limit))
            .bind(("title", query))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        changes: AlbumChangeSet,
    ) -> Result<Option<Self>, Error> {
        Ok(db.update((TABLE_NAME, id)).merge(changes).await?)
    }

    #[instrument()]
    pub async fn read_by_name_and_album_artist<C: Connection>(
        db: &Surreal<C>,
        title: &str,
        album_artists: OneOrMany<Arc<str>>,
    ) -> Result<Option<Self>, Error> {
        if album_artists == OneOrMany::None {
            return Ok(None);
        }

        Ok(db
            .query(read_by_name_and_album_artist())
            .bind(("title", title))
            .bind(("artist", album_artists))
            .await?
            .take(0)?)
    }

    /// Read or create an album by name and album artist
    ///
    /// If the album does not exist, it will be created and added to the artists
    #[instrument()]
    pub async fn read_or_create_by_name_and_album_artist<C: Connection>(
        db: &Surreal<C>,
        title: &str,
        album_artists: OneOrMany<Arc<str>>,
    ) -> Result<Option<Self>, Error> {
        if let Ok(Some(album)) =
            Self::read_by_name_and_album_artist(db, title, album_artists.clone()).await
        {
            Ok(Some(album))
        } else if let Some(album) = Self::create(
            db,
            Self {
                id: Self::generate_id(),
                title: title.into(),
                artist: album_artists.clone(),
                runtime: Duration::from_secs(0),
                release: None,
                song_count: 0,
                discs: 1,
                genre: OneOrMany::None,
            },
        )
        .await?
        {
            // we created a new album made by some artists, so we need to update those artists
            Artist::add_album_to_artists(
                db,
                &Artist::read_or_create_by_names(db, album_artists)
                    .await?
                    .into_iter()
                    .map(|a| a.id)
                    .collect::<Vec<_>>(),
                album.id.clone(),
            )
            .await?;
            Ok(Some(album))
        } else {
            warn!("Failed to create album {}", title);
            Ok(None)
        }
    }

    #[instrument()]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(add_songs())
            .bind(("album", &id))
            .bind(("songs", song_ids))
            .await?;

        Self::repair(db, id).await?;

        Ok(())
    }

    #[instrument()]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> Result<Vec<Song>, Error> {
        Ok(db.query(read_songs()).bind(("album", &id)).await?.take(0)?)
    }

    #[instrument()]
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(remove_songs())
            .bind(("album", &id))
            .bind(("songs", song_ids))
            .await?;
        Self::repair(db, id).await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_artist<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> Result<OneOrMany<Artist>, Error> {
        Ok(db.query(read_artist()).bind(("id", id)).await?.take(0)?)
    }

    /// update counts and runtime
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the album to repair
    ///
    /// # Returns
    ///
    /// Returns a boolean indicating if the album should be removed (if it has no songs left in it)
    #[instrument()]
    pub async fn repair<C: Connection>(db: &Surreal<C>, id: AlbumId) -> Result<bool, Error> {
        // remove or update the album and return
        let songs = Self::read_songs(db, id.clone()).await?;

        Self::update(
            db,
            id,
            AlbumChangeSet {
                song_count: Some(songs.len()),
                runtime: Some(songs.iter().map(|s| s.runtime).sum()),
                ..Default::default()
            },
        )
        .await?;

        Ok(songs.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_utils::init_test_database;

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;

    fn create_album() -> Album {
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

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let created = Album::create(&db, album.clone()).await?;
        assert_eq!(Some(album), created);
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let created = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let read = Album::read(&db, album.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read album"))?;
        assert_eq!(album, read);
        assert_eq!(read, created);
        Ok(())
    }

    #[tokio::test]
    async fn test_update() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let changes = AlbumChangeSet {
            title: Some("New Title".into()),
            ..Default::default()
        };

        let updated = Album::update(&db, album.id.clone(), changes).await?;
        let read = Album::read(&db, album.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read album"))?;

        assert_eq!(read.title, "New Title".into());
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let deleted = Album::delete(&db, album.id.clone()).await?;
        let read = Album::read(&db, album.id.clone()).await?;

        assert_eq!(Some(album), deleted);
        assert_eq!(read, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_name() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let read = Album::read_by_name(&db, "Test Album").await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], album);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let read = Album::read_all(&db).await?;
        assert!(!read.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_name_and_album_artist() -> Result<()> {
        let db = init_test_database().await?;

        let artist = Artist::create(
            &db,
            Artist {
                id: Artist::generate_id(),
                name: "Test Artist".into(),
                runtime: Duration::from_secs(0),
                album_count: 0,
                song_count: 0,
            },
        )
        .await?
        .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        Artist::add_album(&db, artist.id, album.id.clone()).await?;

        let read = Album::read_by_name_and_album_artist(
            &db,
            "Test Album",
            vec!["Test Artist".into()].into(),
        )
        .await?;
        assert_eq!(read, Some(album));
        Ok(())
    }

    #[tokio::test]
    // the test above tests the read branch of this, so here we test the create branch
    async fn test_read_or_create_by_name_and_album_artist() -> Result<()> {
        let db = init_test_database().await?;

        let artist = Artist::create(
            &db,
            Artist {
                id: Artist::generate_id(),
                name: "Test Artist".into(),
                runtime: Duration::from_secs(0),
                album_count: 0,
                song_count: 0,
            },
        )
        .await?
        .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let album = Album {
            id: Album::generate_id(), // <-- this will be different because it's being regenerated, but the rest should be the same
            title: "Test Album".into(),
            artist: vec!["Test Artist".into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let read = Album::read_or_create_by_name_and_album_artist(
            &db,
            "Test Album",
            vec![artist.name.clone()].into(),
        )
        .await?
        .ok_or_else(|| anyhow!("Failed to read or create album"))?;

        assert_eq!(read.title, album.title);
        assert_eq!(read.artist, album.artist);
        assert_eq!(read.runtime, album.runtime);
        assert_eq!(read.release, album.release);
        assert_eq!(read.song_count, album.song_count);
        assert_eq!(read.discs, album.discs);
        assert_eq!(read.genre, album.genre);

        Ok(())
    }

    #[tokio::test]
    async fn test_search() -> Result<()> {
        let db = init_test_database().await?;
        let mut album1 = create_album();
        album1.title = "Foo Bar".into();
        let mut album2 = create_album();
        album2.title = "Foo".into();

        Album::create(&db, album1.clone()).await?;
        Album::create(&db, album2.clone()).await?;

        let found = Album::search(&db, "foo", 2).await?;
        assert_eq!(found.len(), 2);
        assert!(found.contains(&album1));
        assert!(found.contains(&album2));

        let found = Album::search(&db, "bar", 10).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found, vec![album1]);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_songs() -> Result<()> {
        let db = init_test_database().await?;

        let album = create_album();
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: vec!["Test Artist".into()].into(),
            album_artist: vec!["Test Artist".into()].into(),
            album: "Test Album".into(),
            genre: OneOrMany::One("Test Genre".into()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
            analysis: [0.; 20],
        };

        let album = Album::create(&db, album)
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let song = Song::create(&db, song)
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;

        let read = Album::read(&db, album.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read album"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, song.runtime);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_songs() -> Result<()> {
        let db = init_test_database().await?;

        let album = create_album();
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: vec!["Test Artist".into()].into(),
            album_artist: vec!["Test Artist".into()].into(),
            album: "Test Album".into(),
            genre: OneOrMany::One("Test Genre".into()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
            analysis: [0.; 20],
        };

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;

        let read = Album::read_songs(&db, album.id.clone()).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], song);
        Ok(())
    }

    #[tokio::test]
    async fn test_remove_songs() -> Result<()> {
        let db = init_test_database().await?;

        let album = create_album();
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: vec!["Test Artist".into()].into(),
            album_artist: vec!["Test Artist".into()].into(),
            album: "Test Album".into(),
            genre: OneOrMany::One("Test Genre".into()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
            analysis: [0.; 20],
        };

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
        Album::remove_songs(&db, album.id.clone(), &[song.id.clone()]).await?;

        let read = Album::read_songs(&db, album.id.clone()).await?;
        assert_eq!(read.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_artist() -> Result<()> {
        let db = init_test_database().await?;

        let artist = Artist::create(
            &db,
            Artist {
                id: Artist::generate_id(),
                name: "Test Artist".into(),
                runtime: Duration::from_secs(0),
                album_count: 0,
                song_count: 0,
            },
        )
        .await?
        .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let album = create_album();

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        Artist::add_album(&db, artist.id.clone(), album.id.clone()).await?;
        let artist = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;

        let read = Album::read_artist(&db, album.id.clone()).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read.get(0), Some(&artist));
        Ok(())
    }
}
