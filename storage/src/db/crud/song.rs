//! CRUD operations for the song table

use std::path::PathBuf;

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::schemas::{
        album::Album,
        artist::Artist,
        song::{Song, SongChangeSet, SongId, TABLE_NAME},
    },
    errors::Error,
    util::OneOrMany,
};

use super::queries::song::{read_album, read_album_artist, read_artist, read_song_by_path};

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
        let res: Vec<Artist> = db
            .query(read_artist())
            .bind(("id", id))
            .await?
            .take(0)?;

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
            let old_album: Option<Album> = Self::read_album(db, id.clone()).await?;
            let old_album = old_album.ok_or(Error::NotFound)?;

            // find/create the new album
            let new_album = match (&changes.album, &changes.album_artist) {
                (Some(album), Some(album_artist)) => {
                    Album::read_or_create_by_name_and_album_artist(
                        db,
                        album,
                        album_artist.to_owned(),
                    )
                    .await?
                }
                (Some(album), None) => {
                    Album::read_or_create_by_name_and_album_artist(
                        db,
                        album,
                        old_album.artist.clone(),
                    )
                    .await?
                }
                (None, Some(album_artist)) => {
                    // find/create the new album
                    Album::read_or_create_by_name_and_album_artist(
                        db,
                        &old_album.title,
                        album_artist.to_owned(),
                    )
                    .await?
                }
                (None, None) => unreachable!(),
            }
            .ok_or(Error::NotFound)?;

            // remove song from the old album
            Album::remove_songs(db, old_album.id, &[id.clone()]).await?;

            // add song to the new album
            Album::add_songs(db, new_album.id, &[id.clone()]).await?;
        }

        if let Some(artist) = &changes.artist {
            let old_artist: OneOrMany<Artist> = Self::read_artist(db, id.clone()).await?;
            // find/create artists with the new names
            let new_artist = Artist::read_or_create_by_names(db, artist.clone()).await?;

            // remove song from the old artists
            for artist in &old_artist {
                Artist::remove_songs(db, artist.id.clone(), &[id.clone()]).await?;
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{db::init_test_database, test_utils::ulid};

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use surrealdb::sql::Duration;

    fn create_song(ulid: &str) -> Song {
        Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            album_artist: vec![format!("Test Artist {ulid}").into()].into(),
            album: format!("Test Album {ulid}").into(),
            genre: OneOrMany::One(format!("Test Genre {ulid}").into()),
            runtime: Duration::from_secs(120),
            track: None,
            disc: None,
            release_year: None,
            extension: "mp3".into(),
            path: format!("song_{ulid}.mp3").into(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_create(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        let created = Song::create(&db, song.clone()).await?;
        assert_eq!(created, Some(song));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_all(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;
        let songs = Song::read_all(&db).await?;
        assert!(!songs.is_empty());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;
        let read = Song::read(&db, song.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(read, song);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_by_path(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;
        let read = Song::read_by_path(&db, song.path.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(read, song);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_album(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

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

    #[rstest]
    #[tokio::test]
    async fn test_read_artist(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

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

    #[rstest]
    #[tokio::test]
    async fn test_read_album_artist(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

        let artist =
            Artist::read_or_create_by_name(&db, song.album_artist.clone().first().unwrap())
                .await?
                .ok_or_else(|| anyhow!("Album Artist not found/created"))?;
        let album =
            Album::read_or_create_by_name_and_album_artist(&db, &song.album, song.album_artist)
                .await?
                .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
        let artist = Artist::read(&db, artist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Artist not found"))?;
        assert_eq!(
            OneOrMany::One(artist),
            Song::read_album_artist(&db, song.id.clone()).await?
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update_no_repair(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

        // test updating things that don't require relation repair
        let changes = SongChangeSet {
            title: Some(format!("Updated Title {ulid}").into()),
            runtime: Some(Duration::from_secs(10)),
            track: Some(Some(2)),
            disc: Some(Some(2)),
            genre: Some(OneOrMany::One("Updated Genre".into())),
            release_year: Some(Some(2021)),
            extension: Some("flac".into()),
            ..Default::default()
        };
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;

        assert_eq!(updated.title, changes.title.unwrap());
        assert_eq!(updated.runtime, changes.runtime.unwrap());
        assert_eq!(updated.track, changes.track.unwrap());
        assert_eq!(updated.disc, changes.disc.unwrap());
        assert_eq!(updated.genre, changes.genre.unwrap());
        assert_eq!(updated.release_year, changes.release_year.unwrap());
        assert_eq!(updated.extension, changes.extension.unwrap());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update_artist(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

        // note, we need the artist to actually exist in the db and be associated with the song
        let artist = Artist::read_or_create_by_name(&db, song.artist.first().unwrap())
            .await?
            .ok_or_else(|| anyhow!("Artist not found/created"))?;
        Artist::add_songs(&db, artist.id, &[song.id.clone()]).await?;

        let changes = SongChangeSet {
            artist: Some(OneOrMany::One(format!("Updated Artist {ulid}").into())),
            ..Default::default()
        };

        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(updated.artist, changes.artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> =
            Artist::read_or_create_by_names(&db, changes.artist.unwrap())
                .await?
                .into();
        assert_eq!(new_artist, Song::read_artist(&db, song.id.clone()).await?);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update_album_artist(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

        // note, we need the artist to actually exist in the db and be associated with the song
        let artist = Artist::read_or_create_by_name(&db, song.album_artist.first().unwrap())
            .await?
            .ok_or_else(|| anyhow!("Album Artist not found/created"))?;
        // the album must also exist, and be associated with the song and artist
        let album =
            Album::read_or_create_by_name_and_album_artist(&db, &song.album, song.album_artist)
                .await?
                .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
        Artist::add_album(&db, artist.id, album.id.clone()).await?;

        let changes = SongChangeSet {
            album_artist: Some(OneOrMany::One(
                format!("Updated Album Artist {ulid}").into(),
            )),
            ..Default::default()
        };
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(updated.album_artist, changes.album_artist.clone().unwrap());

        // since the new artist didn't exist before, it should have been created
        let new_artist: OneOrMany<_> =
            Artist::read_or_create_by_names(&db, changes.album_artist.unwrap())
                .await?
                .into();
        assert_eq!(
            new_artist,
            Song::read_album_artist(&db, song.id.clone()).await?
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update_album(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let ulid = &ulid;
        let song = create_song(ulid);

        Song::create(&db, song.clone()).await?;

        // note, we need the artist to actually exist in the db and be associated with the song
        let album_artist = Artist::read_or_create_by_name(&db, song.album_artist.first().unwrap())
            .await?
            .ok_or_else(|| anyhow!("Album Artist not found/created"))?;
        // the album must also exist, and be associated with the song and artist
        let album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &song.album,
            song.album_artist.clone(),
        )
        .await?
        .ok_or_else(|| anyhow!("Album not found/created"))?;
        Album::add_songs(&db, album.id.clone(), &[song.id.clone()]).await?;
        Artist::add_album(&db, album_artist.id, album.id.clone()).await?;

        let changes = SongChangeSet {
            album: Some(format!("Updated Album {ulid}").into()),
            ..Default::default()
        };
        let updated = Song::update(&db, song.id.clone(), changes.clone())
            .await?
            .ok_or_else(|| anyhow!("Song not found"))?;
        assert_eq!(updated.album, changes.album.clone().unwrap());

        // since the new album didn't exist before, it should have been created
        let new_album = Album::read_or_create_by_name_and_album_artist(
            &db,
            &changes.album.unwrap(),
            song.album_artist.clone(),
        )
        .await?;
        assert_eq!(new_album, Song::read_album(&db, song.id.clone()).await?);
        assert!(new_album.is_some());
        Ok(())
    }
}
