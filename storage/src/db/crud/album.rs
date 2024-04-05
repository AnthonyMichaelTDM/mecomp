//! CRUD operations for the album table
use std::{sync::Arc, time::Duration};

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

    #[instrument]
    pub async fn read_artists(id: AlbumId) -> Result<Vec<Artist>, Error> {
        Ok(db()
            .await?
            .query("SELECT <-artist_to_album<-artist FROM $id")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn read_by_name_and_album_artist(
        title: &str,
        album_artists: &[Arc<str>],
    ) -> Result<Option<Album>, Error> {
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
        album_artists: &[Arc<str>],
    ) -> Result<Option<AlbumId>, Error> {
        if let Some(album) = Album::read_by_name_and_album_artist(title, album_artists)
            .await?
            .into_iter()
            .next()
        {
            Ok(Some(album.id.clone()))
        } else {
            match Album::create(Album {
                id: Album::generate_id(),
                title: title.into(),
                artist: album_artists.into(),
                runtime: Duration::from_secs(0),
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
                        &Artist::create_or_read_by_names(album_artists)
                            .await?
                            .into_iter()
                            .map(|a| a.id)
                            .collect::<Vec<_>>(),
                        album.id.clone(),
                    )
                    .await?;
                    Ok(Some(album.id.clone()))
                }
                None => {
                    warn!("Failed to create album {}", title);
                    Ok(None)
                }
            }
        }
    }

    #[instrument()]
    pub async fn read_all() -> Result<Vec<Album>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument()]
    pub async fn read(id: AlbumId) -> Result<Option<Album>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
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
            .query(format!("UPDATE type::record($id) MERGE $changes"))
            .bind(("id", &id))
            .bind(("changes", &changes))
            .await?;

        Ok(())
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
            .query("SELECT ->album_to_song FROM ONLY type::record($album)")
            .bind(("album", &id))
            .await?
            .take(0)?)
    }

    #[instrument()]
    pub async fn remove_songs(id: AlbumId, song_ids: &[SongId]) -> Result<(), Error> {
        for song in song_ids {
            let _ = db()
                .await?
                .query("DELETE type::record($album)->album_to_song WHERE out=type::record($song)")
                .query("UPDATE $album SET song_count-=1, runtime-=(SELECT runtime FROM ONLY $song)")
                .bind(("album", &id))
                .bind(("song", song))
                .await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn delete(id: AlbumId) -> Result<(), Error> {
        db().await?
            .query("DELETE ONLY $id")
            .bind(("id", id))
            .await?;
        Ok(())
    }

    /// goes through all the songs in the album and removes any that either don't exist in the database, or don't belong to this album
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the album to repair
    ///
    /// # Returns
    ///
    /// Returns a boolean indicating if the album was removed (if it has no songs left in it)
    ///
    /// TODO: update
    #[instrument()]
    pub async fn repair(id: AlbumId) -> Result<bool, Error> {
        // first, unrelate all the songs that don't belong
        db().await?.query("DELETE $album->album_to_song WHERE out=(song WHERE album == (SELECT title FROM ONLY $album))").bind(("album",id.clone())).await?;

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
