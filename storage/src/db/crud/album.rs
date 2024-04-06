//! CRUD operations for the album table
use std::sync::Arc;

use log::warn;
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::{Album, AlbumChangeSet, AlbumId, TABLE_NAME},
            artist::Artist,
            song::{Song, SongId},
        },
    },
    errors::Error,
    util::OneOrMany,
};

impl Album {
    #[instrument()]
    pub async fn create(album: Album) -> Result<Option<Album>, Error> {
        Ok(db()
            .await?
            .create((TABLE_NAME, album.id.clone()))
            .content(album)
            .await?)
    }

    #[instrument()]
    pub async fn read_all() -> Result<Vec<Album>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument()]
    pub async fn read(id: AlbumId) -> Result<Option<Album>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn delete(id: AlbumId) -> Result<(), Error> {
        let _: Option<Album> = db().await?.delete((TABLE_NAME, id)).await?;
        Ok(())
    }

    #[instrument()]
    pub async fn read_by_name(name: &str) -> Result<Vec<Album>, Error> {
        Ok(db()
            .await?
            .query("SELECT * FROM album WHERE title=$name")
            .bind(("table", TABLE_NAME))
            .bind(("name", name))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn update(id: AlbumId, changes: AlbumChangeSet) -> Result<(), Error> {
        db().await?
            .query(format!("UPDATE $id MERGE $changes"))
            .bind(("id", id))
            .bind(("changes", changes))
            .await?;

        Ok(())
    }

    #[instrument()]
    pub async fn read_by_name_and_album_artist(
        title: &str,
        album_artists: OneOrMany<Arc<str>>,
    ) -> Result<Option<Album>, Error> {
        if let OneOrMany::None = album_artists {
            return Ok(None);
        }

        Ok(db()
            .await?
            .query("SELECT * FROM album WHERE title=$title AND artist=$artist")
            .bind(("table", TABLE_NAME))
            .bind(("title", title))
            .bind(("artist", album_artists))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn read_or_create_by_name_and_album_artist(
        title: &str,
        album_artists: OneOrMany<Arc<str>>,
    ) -> Result<Option<Album>, Error> {
        if let Some(album) = Album::read_by_name_and_album_artist(title, album_artists.clone())
            .await?
            .into_iter()
            .next()
        {
            Ok(Some(album))
        } else {
            match Album::create(Album {
                id: Album::generate_id(),
                title: title.into(),
                artist: album_artists.clone(),
                runtime: surrealdb::sql::Duration::from_secs(0),
                release: None,
                song_count: 0,
                discs: 1,
                genre: OneOrMany::None,
            })
            .await?
            {
                Some(album) => {
                    // we created a new album made by some artists, so we need to update those artists
                    Artist::add_album_to_artists(
                        &Artist::read_or_create_by_names(album_artists)
                            .await?
                            .into_iter()
                            .map(|a| a.id)
                            .collect::<Vec<_>>(),
                        album.id.clone(),
                    )
                    .await?;
                    Ok(Some(album))
                }
                None => {
                    warn!("Failed to create album {}", title);
                    Ok(None)
                }
            }
        }
    }

    #[instrument()]
    pub async fn add_songs(id: AlbumId, song_ids: &[SongId]) -> Result<(), Error> {
        db()
            .await?
            .query("RELATE $album->album_to_song->$songs")
            .query("UPDATE $album SET song_count+=array::len($songs), runtime+=math::sum(SELECT runtime FROM $songs)")
            .bind(("album", &id))
            .bind(("songs", song_ids))
            .await?;

        Ok(())
    }

    #[instrument()]
    pub async fn read_songs(id: AlbumId) -> Result<Vec<Song>, Error> {
        Ok(db()
            .await?
            .query("SELECT ->album_to_song FROM $album")
            .bind(("album", &id))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn remove_songs(id: AlbumId, song_ids: &[SongId]) -> Result<(), Error> {
        for song in song_ids {
            let _ = db()
                .await?
                .query("DELETE $album->album_to_song WHERE out=$song")
                .query(
                    "UPDATE $album SET song_count-=1, runtime-=(SELECT duration FROM ONLY $song)",
                )
                .bind(("album", &id))
                .bind(("song", song))
                .await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn read_artists(id: AlbumId) -> Result<Vec<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT <-artist_to_album<-artist FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    /// update counts and runtime
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the album to repair
    ///
    /// # Returns
    ///
    /// Returns a boolean indicating if the album was removed (if it has no songs left in it)
    #[instrument()]
    pub async fn repair(id: AlbumId) -> Result<bool, Error> {
        // remove or update the album and return
        let songs = Album::read_songs(id.clone()).await?;

        if songs.is_empty() {
            Album::delete(id).await?;
            Ok(true)
        } else {
            Album::update(
                id,
                AlbumChangeSet {
                    runtime: Some(songs.iter().map(|s| s.duration).sum()),
                    song_count: Some(songs.len()),
                    ..Default::default()
                },
            )
            .await?;
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ulid;

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use surrealdb::sql::Duration;

    #[rstest]
    #[tokio::test]
    async fn test_create(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let created = Album::create(album.clone()).await?;
        assert_eq!(Some(album), created);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let created = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        let read = Album::read(album.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read album"))?;
        assert_eq!(album, read);
        assert_eq!(read, created);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        let changes = AlbumChangeSet {
            title: Some(format!("New Title {ulid}").into()),
            ..Default::default()
        };

        Album::update(album.id.clone(), changes).await?;

        let read = Album::read(album.id.clone())
            .await?
            .ok_or(anyhow!("Failed to read album"))?;
        assert_eq!(read.title, format!("New Title {ulid}").into());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_delete(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        Album::delete(album.id.clone()).await?;

        let read = Album::read(album.id.clone()).await?;
        assert_eq!(read, None);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_by_name(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        let read = Album::read_by_name(&format!("Test Album {ulid}")).await?;
        assert_eq!(read.len(), 1);
        assert_eq!(read[0], album);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_all(ulid: String) -> Result<()> {
        let ulid = &ulid;
        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        let read = Album::read_all().await?;
        assert!(read.len() > 0);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_by_name_and_album_artist(ulid: String) -> Result<()> {
        let ulid = &ulid;

        let artist = Artist::create(Artist {
            id: Artist::generate_id(),
            name: format!("Test Artist {ulid}").into(),
            runtime: Duration::from_secs(0),
            album_count: 0,
            song_count: 0,
        })
        .await?
        .ok_or(anyhow!("Failed to create artist"))?;

        let album = Album {
            id: Album::generate_id(),
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let _ = Album::create(album.clone())
            .await?
            .ok_or(anyhow!("Failed to create album"))?;

        Artist::add_album(artist.id, album.id.clone()).await?;

        let read = Album::read_by_name_and_album_artist(
            &format!("Test Album {ulid}"),
            vec![format!("Test Artist {ulid}").into()].into(),
        )
        .await?;
        assert_eq!(read, Some(album));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    // the test above tests the read branch of this, so here we test the create branch
    async fn test_read_or_create_by_name_and_album_artist(ulid: String) -> Result<()> {
        let ulid = &ulid;

        let artist = Artist::create(Artist {
            id: Artist::generate_id(),
            name: format!("Test Artist {ulid}").into(),
            runtime: Duration::from_secs(0),
            album_count: 0,
            song_count: 0,
        })
        .await?
        .ok_or(anyhow!("Failed to create artist"))?;

        let album = Album {
            id: Album::generate_id(), // <-- this will be different because it's being regenerated, but the rest should be the same
            title: format!("Test Album {ulid}").into(),
            artist: vec![format!("Test Artist {ulid}").into()].into(),
            runtime: Duration::from_secs(0),
            release: None,
            song_count: 0,
            discs: 1,
            genre: OneOrMany::None,
        };

        let read = Album::read_or_create_by_name_and_album_artist(
            &format!("Test Album {ulid}"),
            vec![artist.name.clone()].into(),
        )
        .await?
        .ok_or(anyhow!("Failed to read or create album"))?;

        assert_eq!(read.title, album.title);
        assert_eq!(read.artist, album.artist);
        assert_eq!(read.runtime, album.runtime);
        assert_eq!(read.release, album.release);
        assert_eq!(read.song_count, album.song_count);
        assert_eq!(read.discs, album.discs);
        assert_eq!(read.genre, album.genre);

        Ok(())
    }
}
