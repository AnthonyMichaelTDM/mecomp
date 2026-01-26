//! CRUD operations for the album table
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use log::warn;
use surrealdb::{Connection, Surreal};
use surrealqlx::surrql;
use tracing::instrument;

use crate::{
    db::{
        queries::{
            album::{
                add_song, read_artist, read_by_name, read_by_name_and_album_artist, read_songs,
                remove_song,
            },
            generic::read_rand,
        },
        schemas::{
            album::{Album, AlbumBrief, AlbumChangeSet, AlbumId, TABLE_NAME},
            artist::Artist,
            song::{Song, SongId},
        },
    },
    errors::StorageResult,
};
use one_or_many::OneOrMany;

impl Album {
    #[instrument()]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        album: Self,
    ) -> StorageResult<Option<Self>> {
        Ok(db.create(album.id.clone()).content(album).await?)
    }

    #[instrument()]
    pub async fn create_many<C: Connection>(
        db: &Surreal<C>,
        albums: Vec<Self>,
    ) -> StorageResult<Vec<Self>> {
        Ok(db.insert(TABLE_NAME).content(albums).await?)
    }

    #[instrument()]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read_all_brief<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<AlbumBrief>> {
        Ok(db
            .query(surrql!(
                "SELECT type::fields($fields) FROM type::table($table)"
            ))
            .bind(("fields", Self::BRIEF_FIELDS))
            .bind(("table", TABLE_NAME))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn read<C: Connection>(db: &Surreal<C>, id: AlbumId) -> StorageResult<Option<Self>> {
        Ok(db.select(id).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.delete(id).await?)
    }

    #[instrument()]
    pub async fn read_by_name<C: Connection>(
        db: &Surreal<C>,
        name: &str,
    ) -> StorageResult<Vec<Self>> {
        Ok(db
            .query(read_by_name())
            .bind(("name", name.to_string()))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_rand<C: Connection>(
        db: &Surreal<C>,
        limit: usize,
    ) -> StorageResult<Vec<AlbumBrief>> {
        Ok(db
            .query(read_rand())
            .bind(("fields", Self::BRIEF_FIELDS))
            .bind(("table", TABLE_NAME))
            .bind(("limit", limit))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn search<C: Connection>(
        db: &Surreal<C>,
        query: &str,
        limit: usize,
    ) -> StorageResult<Vec<AlbumBrief>> {
        Ok(db
            .query(surrql!("SELECT type::fields($fields),search::score(0) * 2 + search::score(1) * 1 AS relevance FROM album WHERE title @0@ $query OR artist @1@ $query ORDER BY relevance DESC LIMIT $limit"))
            .bind(("fields", Self::BRIEF_FIELDS))
            .bind(("query", query.to_string()))
            .bind(("limit", limit))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        changes: AlbumChangeSet,
    ) -> StorageResult<Option<Self>> {
        Ok(db.update(id).merge(changes).await?)
    }

    #[instrument()]
    pub async fn read_by_name_and_album_artist<C: Connection>(
        db: &Surreal<C>,
        title: &str,
        album_artists: OneOrMany<String>,
    ) -> StorageResult<Option<Self>> {
        if album_artists == OneOrMany::None {
            return Ok(None);
        }

        Ok(db
            .query(read_by_name_and_album_artist())
            .bind(("title", title.to_string()))
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
        album_artists: OneOrMany<String>,
    ) -> StorageResult<Option<Self>> {
        if let Ok(Some(album)) =
            Self::read_by_name_and_album_artist(db, title, album_artists.clone()).await
        {
            return Ok(Some(album));
        }

        let album = Self {
            id: Self::generate_id(),
            title: title.into(),
            artist: album_artists.clone(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        if let Some(album) = Self::create(db, album).await? {
            // we created a new album made by some artists, so we need to update those artists
            Artist::add_album_to_artists(
                db,
                Artist::read_or_create_by_names(db, album_artists)
                    .await?
                    .into_iter()
                    .map(|a| a.id)
                    .collect::<Vec<_>>(),
                album.id.clone(),
            )
            .await?;
            Ok(Some(album))
        } else {
            warn!("Failed to create album {title}");
            Ok(None)
        }
    }

    /// Read or create multiple albums by their names and artists in bulk.
    /// This is much more efficient than calling `read_or_create_by_name_and_album_artist` repeatedly.
    ///
    /// Returns a `HashMap` mapping `(title, album_artist)` tuples to their corresponding `Album`s.
    #[instrument()]
    pub async fn bulk_read_or_create_by_name_and_album_artist<C: Connection>(
        db: &Surreal<C>,
        albums: HashSet<(String, OneOrMany<String>)>,
    ) -> StorageResult<HashMap<(String, OneOrMany<String>), AlbumId>> {
        // figure out which albums already exist
        let existing: Vec<Self> = db
            .query(surrql!(
                "SELECT * FROM album WHERE [title, artist] IN $album_tuples"
            ))
            .bind(("album_tuples", albums.clone()))
            .await?
            .take(0)?;

        let mut result: HashMap<(String, OneOrMany<String>), AlbumId> = existing
            .into_iter()
            .map(|a| ((a.title, a.artist), a.id))
            .collect();

        // find which albums are missing
        let missing: Vec<Self> = albums
            .into_iter()
            .filter(|key| !result.contains_key(key))
            .map(|(title, artist)| Self {
                id: Self::generate_id(),
                title,
                artist,
                release: None,
                runtime: Duration::from_secs(0),
                song_count: 0,
                discs: 1,
                genre: OneOrMany::None,
            })
            .collect();

        // create missing albums in bulk
        if !missing.is_empty() {
            let created_albums = Self::create_many(db, missing).await?;
            for album in created_albums {
                // add album to its artists
                Artist::add_album_to_artists(
                    db,
                    Artist::read_or_create_by_names(db, album.artist.clone())
                        .await?
                        .into_iter()
                        .map(|a| a.id)
                        .collect::<Vec<_>>(),
                    album.id.clone(),
                )
                .await?;

                result.insert((album.title.clone(), album.artist.clone()), album.id);
            }
        }

        Ok(result)
    }

    #[instrument()]
    pub async fn add_song<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        song_id: SongId,
    ) -> StorageResult<()> {
        db.query(add_song())
            .bind(("album", id))
            .bind(("song", song_id))
            .await?;

        Ok(())
    }

    #[instrument()]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> StorageResult<Vec<Song>> {
        Ok(db.query(read_songs()).bind(("album", id)).await?.take(0)?)
    }

    #[instrument()]
    /// Remove a song from an album
    pub async fn remove_song<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
        song_id: SongId,
    ) -> StorageResult<()> {
        db.query(remove_song())
            .bind(("album", id))
            .bind(("song", song_id))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_artist<C: Connection>(
        db: &Surreal<C>,
        id: AlbumId,
    ) -> StorageResult<OneOrMany<Artist>> {
        Ok(db.query(read_artist()).bind(("id", id)).await?.take(0)?)
    }

    /// Deletes all orphaned albums from the database
    ///
    /// An orphaned album is an album that has no songs in it
    #[instrument]
    pub async fn delete_orphaned<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db
            .query(surrql!(
                "DELETE FROM album WHERE type::int(song_count) = 0 RETURN BEFORE"
            ))
            .await?
            .take(0)?)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_utils::init_test_database;

    use anyhow::{Result, anyhow};
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

        assert_eq!(read.title, "New Title");
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
    async fn test_read_rand() -> Result<()> {
        let db = init_test_database().await?;
        let album1 = create_album();
        let mut album2 = create_album();
        album2.title = "Another Test Album".into();

        let album1 = Album::create(&db, album1.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?
            .into();
        let album2 = Album::create(&db, album2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?
            .into();

        // n = # records
        let read = Album::read_rand(&db, 2).await?;
        assert_eq!(read.len(), 2);
        assert!(read.contains(&album1) && read.contains(&album2));
        // n > # records
        let read = Album::read_rand(&db, 3).await?;
        assert_eq!(read.len(), 2);
        assert!(read.contains(&album1) && read.contains(&album2));
        // n < # records
        let read = Album::read_rand(&db, 1).await?;
        assert_eq!(read.len(), 1);
        assert!(read.contains(&album1) || read.contains(&album2));
        // n == 0
        let read = Album::read_rand(&db, 0).await?;
        assert_eq!(read.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_album();

        let album = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        let read = Album::read_all(&db).await?;
        assert!(!read.is_empty());
        assert_eq!(read, vec![album.clone()]);

        let read = Album::read_all_brief(&db).await?;
        assert!(!read.is_empty());
        assert_eq!(read, vec![album.into()]);
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
    async fn test_bulk_read_or_create_by_name_and_album_artist() -> Result<()> {
        let db = init_test_database().await?;

        Album::read_or_create_by_name_and_album_artist(&db, "0", vec!["0".to_string()].into())
            .await?;
        Album::read_or_create_by_name_and_album_artist(&db, "0", vec!["1".to_string()].into())
            .await?;
        Album::read_or_create_by_name_and_album_artist(&db, "1", vec!["0".to_string()].into())
            .await?;
        // one of them already exists
        Album::read_or_create_by_name_and_album_artist(&db, "1", vec!["1".to_string()].into())
            .await?;

        let read: Vec<Album> = db
            .query(surrql!(
                "SELECT * FROM album WHERE [title, artist] IN $album_tuples"
            ))
            .bind(("album_tuples", vec![("1".to_string(), "1".to_string())]))
            .await?
            .take(0)?;
        assert_eq!(read.len(), 1);

        let albums = vec![
            ("1".to_string(), vec!["1".to_string()].into()),
            ("2".to_string(), vec!["2".to_string()].into()),
            ("3".to_string(), vec!["3".to_string()].into()),
        ]
        .into_iter()
        .collect();

        let result = Album::bulk_read_or_create_by_name_and_album_artist(&db, albums).await?;

        assert_eq!(result.len(), 3);
        for ((title, artist), album_id) in result {
            assert!(title == "1" || title == "2" || title == "3");
            assert_eq!(artist, OneOrMany::One(Box::new(title.clone())).into());
            let read = Album::read(&db, album_id.clone()).await?;
            assert!(read.is_some());
            let read = Album::read_artist(&db, album_id).await?;
            assert!(read.is_one());
            let read = read.get(0).map(|a| a.name.clone());
            assert_eq!(read, Some(title));
        }

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
        assert!(found.contains(&album1.clone().into()));
        assert!(found.contains(&album2.into()));

        let found = Album::search(&db, "bar", 10).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found, vec![album1.into()]);
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
            genre: "Test Genre".to_string().into(),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
        };

        let album = Album::create(&db, album)
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let song = Song::create(&db, song)
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_song(&db, album.id.clone(), song.id.clone()).await?;

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
            genre: "Test Genre".to_string().into(),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
        };

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_song(&db, album.id.clone(), song.id.clone()).await?;

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
            genre: "Test Genre".to_string().into(),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: "song.mp3".into(),
        };

        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_song(&db, album.id.clone(), song.id.clone()).await?;
        let read = Album::read_songs(&db, album.id.clone()).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], song);

        Album::remove_song(&db, album.id.clone(), song.id.clone()).await?;
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
