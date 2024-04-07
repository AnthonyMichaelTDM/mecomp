//! CRUD operations for the artist table
use std::sync::Arc;

use surrealdb::sql::Duration;
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::{Album, AlbumId},
            artist::{Artist, ArtistChangeSet, ArtistId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::Error,
    util::OneOrMany,
};

impl Artist {
    #[instrument]
    pub async fn create(artist: Artist) -> Result<Option<Artist>, Error> {
        Ok(db()
            .await?
            .create((TABLE_NAME, artist.id.clone()))
            .content(artist)
            .await?)
    }

    #[instrument]
    pub async fn read_or_create_by_name(name: &str) -> Result<Option<Artist>, Error> {
        if let Some(artist) = Artist::read_by_name(name).await? {
            Ok(Some(artist))
        } else {
            Artist::create(Artist {
                id: Artist::generate_id(),
                name: name.into(),
                song_count: 0,
                album_count: 0,
                runtime: Duration::from_secs(0),
            })
            .await
        }
    }

    #[instrument]
    pub async fn read_or_create_by_names(names: OneOrMany<Arc<str>>) -> Result<Vec<Artist>, Error> {
        let mut artists = Vec::with_capacity(names.len());
        for name in names.iter() {
            if let Some(id) = Artist::read_or_create_by_name(name).await? {
                artists.push(id);
            }
        }
        Ok(artists)
    }

    #[instrument]
    pub async fn read_by_name(name: &str) -> Result<Option<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM artist WHERE name = $name LIMIT 1")
            .bind(("name", name))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_by_names(names: &[Arc<str>]) -> Result<Vec<Artist>, Error> {
        // select artists records whose `name` field is in $names
        Ok(db()
            .await?
            .query("SELECT * FROM artist WHERE name IN $names")
            .bind(("names", names))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read_all() -> Result<Vec<Artist>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: ArtistId) -> Result<Option<Artist>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_one_or_many(ids: OneOrMany<ArtistId>) -> Result<OneOrMany<Artist>, Error> {
        match ids {
            OneOrMany::One(id) => Ok(Artist::read(id).await?.into()),
            OneOrMany::Many(ids) => Artist::read_many(ids).await.map(|v| v.into()),
            OneOrMany::None => Ok(OneOrMany::None),
        }
    }

    #[instrument]
    pub async fn read_many(ids: Vec<ArtistId>) -> Result<Vec<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM $ids")
            .bind(("ids", ids))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn update(id: ArtistId, changes: ArtistChangeSet) -> Result<(), Error> {
        let _: Option<Artist> = db().await?.update((TABLE_NAME, id)).merge(changes).await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_albums(id: ArtistId) -> Result<Vec<Album>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM $id->artist_to_album->album")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn add_album(id: ArtistId, album_id: AlbumId) -> Result<(), Error> {
        db().await?
            // relate this artist to the album
            .query("RELATE $id->artist_to_album->$album;")
            // relate this artist to the songs in the album
            // .query("RELATE $id->artist_to_song->(SELECT ->album_to_song<-album FROM $album);")
            .bind(("id", &id))
            .bind(("album", &album_id))
            .await?;
        // update runtime, and song/album count
        Artist::repair(id.clone()).await?;
        Ok(())
    }

    #[instrument]
    pub async fn add_album_to_artists(ids: &[ArtistId], album_id: AlbumId) -> Result<(), Error> {
        db().await?
            // relate this artist to the album
            .query("RELATE $ids->artist_to_album->$album")
            .bind(("ids", &ids))
            .bind(("album", &album_id))
            .await?;
        for id in ids {
            Artist::repair(id.clone()).await?;
        }
        Ok(())
    }

    #[instrument]
    /// gets all the songs associated with an artist, either directly or through an album
    pub async fn add_songs(id: ArtistId, songs: &[SongId]) -> Result<(), Error> {
        db().await?
            // relate this artist to these songs
            .query("RELATE $id->artist_to_song->$songs")
            .bind(("id", &id))
            .bind(("songs", songs))
            .await?;
        Artist::repair(id.clone()).await?;
        Ok(())
    }

    #[instrument]
    pub async fn remove_songs(id: ArtistId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await?
            .query("DELETE $artist->artist_to_song WHERE out IN $songs")
            .bind(("artist", &id))
            .bind(("songs", song_ids))
            .await?;
        Artist::repair(id.clone()).await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_songs(id: ArtistId) -> Result<Vec<Song>, Error> {
        Ok(db().await?
            .query("RETURN array::union((SELECT * FROM $artist->artist_to_song->song), (SELECT * FROM $artist->artist_to_album->album->album_to_song->song))")
            .bind(("artist", id)).await?.take(0)?)
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
    pub async fn repair(id: ArtistId) -> Result<bool, Error> {
        let albums: Vec<Album> = Artist::read_albums(id.clone()).await?;
        let songs: Vec<Song> = Artist::read_songs(id.clone()).await?;

        Artist::update(
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
    use std::path::PathBuf;

    use super::*;
    use crate::test_utils::ulid;

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use surrealdb::sql::Duration;

    fn create_artist(ulid: &str) -> Artist {
        Artist {
            id: Artist::generate_id(),
            name: format!("Test Artist {ulid}").into(),
            runtime: Duration::from_secs(0),
            album_count: 0,
            song_count: 0,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_create(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);

        let created = Artist::create(artist.clone()).await?;
        assert_eq!(Some(artist), created);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);

        let created = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read(artist.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read artist"))?;
        assert_eq!(artist, read);
        assert_eq!(read, created);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_one_or_many(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let mut artist2 = create_artist(ulid);
        artist2.name = format!("Test Artist 2 {ulid}").into();

        // test None
        let read = Artist::read_one_or_many(OneOrMany::None).await?;
        assert_eq!(read, OneOrMany::None);

        // test One
        let created = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let read = Artist::read_one_or_many(OneOrMany::One(artist.id.clone())).await?;
        assert_eq!(read, OneOrMany::One(created.clone()));

        // test Many
        let created2 = Artist::create(artist2.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let read =
            Artist::read_one_or_many(OneOrMany::Many(vec![artist.id.clone(), artist2.id.clone()]))
                .await?;
        assert_eq!(read, OneOrMany::Many(vec![created, created2]));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_many(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let mut artist2 = create_artist(ulid);
        artist2.name = format!("Test Artist 2 {ulid}").into();

        let created = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let created2 = Artist::create(artist2.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read_many(vec![artist.id.clone(), artist2.id.clone()]).await?;
        assert_eq!(read, vec![created, created2]);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);

        let _ = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let changes = ArtistChangeSet {
            name: Some(format!("New Name {ulid}").into()),
            ..Default::default()
        };

        Artist::update(artist.id.clone(), changes).await?;

        let read = Artist::read(artist.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read artist"))?;
        assert_eq!(read.name, format!("New Name {ulid}").into());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_by_name(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = create_artist(ulid);

        let _ = Artist::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read_by_name(&format!("Test Artist {ulid}")).await?;
        assert_eq!(read, Some(album));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    /// read path tested in `test_read_by_name`, so we only need to test the create path
    async fn test_read_or_create_by_name(ulid: String) -> Result<()> {
        let ulid = &ulid;

        let created = Artist::read_or_create_by_name(&format!("Test Artist {ulid}"))
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read_by_name(&format!("Test Artist {ulid}")).await?;
        assert_eq!(read, Some(created));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_by_names(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = create_artist(ulid);
        let mut album2 = create_artist(ulid);
        album2.name = format!("Test Artist 2 {ulid}").into();

        let _ = Artist::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let _ = Artist::create(album2.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read_by_names(&[
            format!("Test Artist {ulid}").into(),
            format!("Test Artist 2 {ulid}").into(),
        ])
        .await?;

        assert_eq!(read.len(), 2);

        if read[0].name != album.name {
            assert_eq!(read[1], album);
            assert_eq!(read[0], album2);
        } else {
            assert_eq!(read[0], album);
            assert_eq!(read[1], album2);
        }

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    /// read path tested in `test_read_by_names`, so we only need to test the create path
    async fn test_read_or_create_by_names(ulid: String) -> Result<()> {
        let ulid = &ulid;

        let created = Artist::read_or_create_by_names(OneOrMany::Many(vec![
            format!("Test Artist {ulid}").into(),
            format!("Test Artist 2 {ulid}").into(),
        ]))
        .await?;

        let read = Artist::read_by_names(&[
            format!("Test Artist {ulid}").into(),
            format!("Test Artist 2 {ulid}").into(),
        ])
        .await?;

        assert_eq!(read.len(), 2);

        assert_eq!(read, created);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_all(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = create_artist(ulid);

        let _ = Artist::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;

        let read = Artist::read_all().await?;
        assert!(read.len() > 0);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_albums(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            song_count: 4,
            runtime: Duration::from_secs(5),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        Artist::add_album(artist.id.clone(), album.id.clone()).await?;

        let read = Artist::read_albums(artist.id.clone()).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], album);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_add_album(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            song_count: 0,
            runtime: Duration::from_secs(0),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };

        let _ = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;
        let _ = Song::create(song.clone())
            .await?
            .ok_or(anyhow!("Failed to create song"))?;

        Album::add_songs(album.id.clone(), &[song.id.clone()]).await?;
        Artist::add_album(artist.id.clone(), album.id.clone()).await?;

        let read = Artist::read(artist.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read artist"))?;

        assert_eq!(read.album_count, 1);
        assert_eq!(read.runtime, Duration::from_secs(5));
        assert_eq!(read.song_count, 1);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_add_album_to_artists(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let mut artist2 = create_artist(ulid);
        artist2.name = format!("Test Artist 2 {ulid}").into();
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: OneOrMany::Many(vec![artist.name.clone(), artist2.name.clone()]),
            song_count: 0,
            runtime: Duration::from_secs(0),
            release: None,
            discs: 1,
            genre: OneOrMany::None,
        };
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };

        let _ = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let _ = Artist::create(artist2.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;
        let _ = Song::create(song.clone())
            .await?
            .ok_or(anyhow!("Failed to create song"))?;

        Album::add_songs(album.id.clone(), &[song.id.clone()]).await?;
        Artist::add_album_to_artists(&[artist.id.clone(), artist2.id.clone()], album.id.clone())
            .await?;

        let read = Artist::read_many(vec![artist.id.clone(), artist2.id.clone()]).await?;

        for artist in read {
            assert_eq!(artist.album_count, 1);
            assert_eq!(artist.runtime, Duration::from_secs(5));
            assert_eq!(artist.song_count, 1);
        }

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_add_songs(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };

        let artist = Artist::create(artist)
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let song = Song::create(song)
            .await?
            .ok_or(anyhow!("Failed to create song"))?;

        Artist::add_songs(artist.id.clone(), &[song.id.clone()]).await?;

        let read = Artist::read(artist.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read artist"))?;

        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, Duration::from_secs(5));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_remove_songs(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let artist = create_artist(ulid);
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(artist.name.clone()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(artist.name.clone()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };

        let artist = Artist::create(artist.clone())
            .await?
            .ok_or(anyhow!("Failed to create artist"))?;
        let song = Song::create(song.clone())
            .await?
            .ok_or(anyhow!("Failed to create song"))?;

        Artist::add_songs(artist.id.clone(), &[song.id.clone()]).await?;

        Artist::remove_songs(artist.id.clone(), &[song.id.clone()]).await?;

        let read = Artist::read(artist.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read artist"))?;

        assert_eq!(read.runtime, Duration::from_secs(0));
        assert_eq!(read.song_count, 0);

        Ok(())
    }
}
