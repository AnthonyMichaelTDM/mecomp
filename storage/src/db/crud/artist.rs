//! CRUD operations for the artist table
use std::time::Duration;

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::{
            artist::{
                add_album, add_album_to_artists, add_songs, read_albums, read_by_name,
                read_by_names, read_many, read_songs, remove_songs,
            },
            generic::full_text_search,
        },
        schemas::{
            album::{Album, AlbumId},
            artist::{Artist, ArtistChangeSet, ArtistId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::StorageResult,
};
use one_or_many::OneOrMany;

impl Artist {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        artist: Self,
    ) -> StorageResult<Option<Self>> {
        Ok(db.create(artist.id.clone()).content(artist).await?)
    }

    #[instrument]
    pub async fn read_or_create_by_name<C: Connection>(
        db: &Surreal<C>,
        name: &str,
    ) -> StorageResult<Option<Self>> {
        if let Ok(Some(artist)) = Self::read_by_name(db, name).await {
            Ok(Some(artist))
        } else {
            Self::create(
                db,
                Self {
                    id: Self::generate_id(),
                    name: name.into(),
                    song_count: 0,
                    album_count: 0,
                    runtime: Duration::from_secs(0),
                },
            )
            .await
        }
    }

    #[instrument]
    pub async fn read_or_create_by_names<C: Connection>(
        db: &Surreal<C>,
        names: OneOrMany<String>,
    ) -> StorageResult<Vec<Self>> {
        let mut artists = Vec::with_capacity(names.len());
        for name in &names {
            if let Some(id) = Self::read_or_create_by_name(db, name).await? {
                artists.push(id);
            }
        }
        Ok(artists)
    }

    #[instrument]
    pub async fn read_by_name<C: Connection>(
        db: &Surreal<C>,
        name: &str,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .query(read_by_name())
            .bind(("name", name.to_string()))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_by_names<C: Connection>(
        db: &Surreal<C>,
        names: Vec<String>,
    ) -> StorageResult<Vec<Self>> {
        // select artists records whose `name` field is in $names
        Ok(db
            .query(read_by_names())
            .bind(("names", names))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(db: &Surreal<C>, id: ArtistId) -> StorageResult<Option<Self>> {
        Ok(db.select(id).await?)
    }

    #[instrument]
    pub async fn read_one_or_many<C: Connection>(
        db: &Surreal<C>,
        ids: OneOrMany<ArtistId>,
    ) -> StorageResult<OneOrMany<Self>> {
        match ids {
            OneOrMany::One(id) => Ok(Self::read(db, id).await?.into()),
            OneOrMany::Many(ids) => Self::read_many(db, ids).await.map(std::convert::Into::into),
            OneOrMany::None => Ok(OneOrMany::None),
        }
    }

    #[instrument]
    pub async fn read_many<C: Connection>(
        db: &Surreal<C>,
        ids: Vec<ArtistId>,
    ) -> StorageResult<Vec<Self>> {
        Ok(db.query(read_many()).bind(("ids", ids)).await?.take(0)?)
    }

    #[instrument]
    pub async fn search<C: Connection>(
        db: &Surreal<C>,
        query: &str,
        limit: i64,
    ) -> StorageResult<Vec<Self>> {
        Ok(db
            .query(full_text_search(TABLE_NAME, "name", limit))
            .bind(("name", query.to_string()))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
        changes: ArtistChangeSet,
    ) -> StorageResult<Option<Self>> {
        Ok(db.update(id).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.delete(id).await?)
    }

    #[instrument]
    pub async fn read_albums<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
    ) -> StorageResult<Vec<Album>> {
        Ok(db.query(read_albums()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn add_album<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
        album_id: AlbumId,
    ) -> StorageResult<()> {
        db
            // relate this artist to the album
            .query(add_album())
            // relate this artist to the songs in the album
            // .query("RELATE $id->artist_to_song->(SELECT ->album_to_song<-album FROM $album)")
            .bind(("id", id.clone()))
            .bind(("album", album_id))
            .await?;
        // update runtime, and song/album count
        Self::repair(db, id).await?;
        Ok(())
    }

    #[instrument]
    pub async fn add_album_to_artists<C: Connection>(
        db: &Surreal<C>,
        ids: Vec<ArtistId>,
        album_id: AlbumId,
    ) -> StorageResult<()> {
        db
            // relate this artist to the album
            .query(add_album_to_artists())
            .bind(("ids", ids.clone()))
            .bind(("album", album_id))
            .await?;
        for id in ids {
            Self::repair(db, id.clone()).await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
        songs: Vec<SongId>,
    ) -> StorageResult<()> {
        db
            // relate this artist to these songs
            .query(add_songs())
            .bind(("id", id.clone()))
            .bind(("songs", songs))
            .await?;
        Self::repair(db, id).await?;
        Ok(())
    }

    #[instrument]
    /// removes songs from an artist's list of songs
    ///
    /// # Returns
    ///
    /// * `bool` - whether the artist should be removed or not (if it has no songs or albums, it should be removed)
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
        song_ids: Vec<SongId>,
    ) -> StorageResult<bool> {
        db.query(remove_songs())
            .bind(("artist", id.clone()))
            .bind(("songs", song_ids))
            .await?;

        Self::repair(db, id).await
    }

    #[instrument]
    /// gets all the songs associated with an artist, either directly or through an album
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        id: ArtistId,
    ) -> StorageResult<Vec<Song>> {
        Ok(db.query(read_songs()).bind(("artist", id)).await?.take(0)?)
    }

    /// updates the album count, song count, and runtime of the artist
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the artist to repair
    ///
    /// # Returns
    ///
    /// * `bool` - whether the artist should be removed or not (if it has no songs or albums, it should be removed)
    #[instrument]
    pub async fn repair<C: Connection>(db: &Surreal<C>, id: ArtistId) -> StorageResult<bool> {
        let albums: Vec<Album> = Self::read_albums(db, id.clone()).await?;
        let songs: Vec<Song> = Self::read_songs(db, id.clone()).await?;

        Self::update(
            db,
            id.clone(),
            ArtistChangeSet {
                album_count: Some(albums.len()),
                song_count: Some(songs.len()),
                runtime: Some(songs.iter().map(|s| s.runtime).sum()),
                ..Default::default()
            },
        )
        .await?;

        Ok(albums.is_empty() && songs.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use super::*;
    use crate::test_utils::init_test_database;

    use anyhow::{Result, anyhow};
    use pretty_assertions::assert_eq;

    fn create_artist() -> Artist {
        Artist {
            id: Artist::generate_id(),
            name: "Test Artist".into(),
            runtime: Duration::from_secs(0),
            album_count: 0,
            song_count: 0,
        }
    }

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();

        let created = Artist::create(&db, artist.clone()).await?;
        assert_eq!(Some(artist), created);
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();

        let created = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;
        assert_eq!(artist, read);
        assert_eq!(read, created);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_one_or_many() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let mut artist2 = create_artist();
        artist2.name = "Test Artist 2".to_string();

        // test None
        let read = Artist::read_one_or_many(&db, OneOrMany::None).await?;
        assert_eq!(read, OneOrMany::None);

        // test One
        let created = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let read = Artist::read_one_or_many(&db, OneOrMany::One(artist.id.clone())).await?;
        assert_eq!(read, OneOrMany::One(created.clone()));

        // test Many
        let created2 = Artist::create(&db, artist2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let read = Artist::read_one_or_many(
            &db,
            OneOrMany::Many(vec![artist.id.clone(), artist2.id.clone()]),
        )
        .await?;
        assert_eq!(read, OneOrMany::Many(vec![created, created2]));

        Ok(())
    }

    #[tokio::test]
    async fn test_read_many() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let mut artist2 = create_artist();
        artist2.name = "Test Artist 2".to_string();

        let created = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let created2 = Artist::create(&db, artist2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read = Artist::read_many(&db, vec![artist.id.clone(), artist2.id.clone()]).await?;
        assert_eq!(read, vec![created, created2]);
        Ok(())
    }

    #[tokio::test]
    async fn test_search() -> Result<()> {
        let db = init_test_database().await?;
        let mut artist1 = create_artist();
        artist1.name = "Foo Bar".into();
        let mut artist2 = create_artist();
        artist2.name = "Foo".into();

        Artist::create(&db, artist1.clone()).await?;
        Artist::create(&db, artist2.clone()).await?;

        let found = Artist::search(&db, "foo", 2).await?;
        assert_eq!(found.len(), 2);
        assert!(found.contains(&artist1));
        assert!(found.contains(&artist2));

        let found = Artist::search(&db, "bar", 1).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found, vec![artist1]);
        Ok(())
    }

    #[tokio::test]
    async fn test_update() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let changes = ArtistChangeSet {
            name: Some("New Name".to_string()),
            ..Default::default()
        };

        let updated = Artist::update(&db, artist.id.clone(), changes).await?;
        let read = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;

        assert_eq!(read.name, "New Name".to_string());
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let deleted = Artist::delete(&db, artist.id.clone()).await?;
        let read = Artist::read(&db, artist.id.clone()).await?;

        assert_eq!(deleted, Some(artist));
        assert_eq!(read, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_name() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_artist();

        let _ = Artist::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read = Artist::read_by_name(&db, "Test Artist").await?;
        assert_eq!(read, Some(album));
        Ok(())
    }

    #[tokio::test]
    /// read path tested in `test_read_by_name`, so we only need to test the create path
    async fn test_read_or_create_by_name() -> Result<()> {
        let db = init_test_database().await?;

        let created = Artist::read_or_create_by_name(&db, "Test Artist")
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read = Artist::read_by_name(&db, "Test Artist").await?;
        assert_eq!(read, Some(created));
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_names() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_artist();
        let mut album2 = create_artist();
        album2.name = "Test Artist 2".into();

        let _ = Artist::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Artist::create(&db, album2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read =
            Artist::read_by_names(&db, vec!["Test Artist".into(), "Test Artist 2".into()]).await?;

        assert_eq!(read.len(), 2);

        if read[0].name == album.name {
            assert_eq!(read[0], album);
            assert_eq!(read[1], album2);
        } else {
            assert_eq!(read[1], album);
            assert_eq!(read[0], album2);
        }

        Ok(())
    }

    #[tokio::test]
    /// read path tested in `test_read_by_names`, so we only need to test the create path
    async fn test_read_or_create_by_names() -> Result<()> {
        let db = init_test_database().await?;

        let created = Artist::read_or_create_by_names(
            &db,
            OneOrMany::Many(vec!["Test Artist".into(), "Test Artist 2".into()]),
        )
        .await?;

        let read =
            Artist::read_by_names(&db, vec!["Test Artist".into(), "Test Artist 2".into()]).await?;

        assert_eq!(read.len(), 2);

        assert_eq!(read, created);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let album = create_artist();

        let _ = Artist::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;

        let read = Artist::read_all(&db).await?;
        assert!(!read.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_read_albums() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let album = Album {
            id: Album::generate_id(),
            title: "Test Album".into(),
            artist: OneOrMany::One(artist.name.clone()),
            song_count: 4,
            runtime: Duration::from_secs(5),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;

        Artist::add_album(&db, artist.id.clone(), album.id.clone()).await?;

        let read = Artist::read_albums(&db, artist.id.clone()).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], album);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_album() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let album = Album {
            id: Album::generate_id(),
            title: "Test Album".into(),
            artist: OneOrMany::One(artist.name.clone()),
            song_count: 0,
            runtime: Duration::from_secs(0),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song.mp3"),
        };

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;
        Artist::add_album(&db, artist.id.clone(), album.id.clone()).await?;

        let read = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;

        assert_eq!(read.album_count, 1);
        assert_eq!(read.runtime, song.runtime);
        assert_eq!(read.song_count, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_album_to_artists() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let mut artist2 = create_artist();
        artist2.name = "Test Artist 2".into();
        let album = Album {
            id: Album::generate_id(),
            title: "Test Album".into(),
            artist: OneOrMany::Many(vec![artist.name.clone(), artist2.name.clone()]),
            song_count: 0,
            runtime: Duration::from_secs(0),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".to_string(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album ".to_string(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song.mp3"),
        };

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Artist::create(&db, artist2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(&db, album.id.clone(), vec![song.id.clone()]).await?;
        Artist::add_album_to_artists(
            &db,
            vec![artist.id.clone(), artist2.id.clone()],
            album.id.clone(),
        )
        .await?;

        let read = Artist::read_many(&db, vec![artist.id.clone(), artist2.id.clone()]).await?;

        for artist in read {
            assert_eq!(artist.album_count, 1);
            assert_eq!(artist.runtime, song.runtime);
            assert_eq!(artist.song_count, 1);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_add_songs() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song.mp3"),
        };

        let artist = Artist::create(&db, artist)
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let song = Song::create(&db, song)
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Artist::add_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?;

        let read = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;

        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, song.runtime);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_songs() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let song = Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song.mp3"),
        };

        let artist = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let song = Song::create(&db, song.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Artist::add_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?;

        assert!(Artist::remove_songs(&db, artist.id.clone(), vec![song.id.clone()]).await?);

        let read = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to read artist"))?;

        assert_eq!(read.runtime, Duration::from_secs(0));
        assert_eq!(read.song_count, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_songs() -> Result<()> {
        let db = init_test_database().await?;
        let artist = create_artist();
        let album = Album {
            id: Album::generate_id(),
            title: "Test Album".into(),
            artist: OneOrMany::One(artist.name.clone()),
            song_count: 4,
            runtime: Duration::from_secs(5),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };
        // in an album by the artist
        let song1 = Song {
            id: Song::generate_id(),
            title: "Test Song 1".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song.mp3"),
        };
        // directly by the artist
        let song2 = Song {
            id: Song::generate_id(),
            title: "Test Song 2".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(2),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song_2.mp3"),
        };
        // both
        let song3 = Song {
            id: Song::generate_id(),
            title: "Test Song 3".into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: "Test Album".into(),
            runtime: Duration::from_secs(5),
            track: Some(3),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from("song_3.mp3"),
        };

        let _ = Artist::create(&db, artist.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create artist"))?;
        let _ = Album::create(&db, album.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create album"))?;
        let _ = Song::create(&db, song1.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;
        let _ = Song::create(&db, song2.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;
        let _ = Song::create(&db, song3.clone())
            .await?
            .ok_or_else(|| anyhow!("Failed to create song"))?;

        Album::add_songs(
            &db,
            album.id.clone(),
            vec![song1.id.clone(), song3.id.clone()],
        )
        .await?;
        Artist::add_album(&db, artist.id.clone(), album.id.clone()).await?;
        Artist::add_songs(
            &db,
            artist.id.clone(),
            vec![song2.id.clone(), song3.id.clone()],
        )
        .await?;

        let mut read = Artist::read_songs(&db, artist.id.clone()).await?;

        // we want to check that all the songs are there, but the order will be arbitrary.
        // so we'll just sort them by title and compare that way
        read.sort_by(|a, b| a.title.cmp(&b.title));
        assert_eq!(vec![song1, song2, song3], read);
        Ok(())
    }
}
