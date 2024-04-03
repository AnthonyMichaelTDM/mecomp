//! CRUD operations for the artist table
use std::{sync::Arc, time::Duration};

use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::{Album, AlbumId},
            artist::{Artist, ArtistId, TABLE_NAME},
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
    pub async fn create_or_read_by_name(name: &str) -> Result<Option<Artist>, Error> {
        if let Some(artist) = db()
            .await?
            .select(TABLE_NAME)
            .await?
            .into_iter()
            .find(|x: &Artist| x.name.as_ref() == name)
        {
            Ok(Some(artist))
        } else {
            Artist::create(Artist {
                id: Artist::generate_id(),
                name: name.into(),
                songs: vec![].into_boxed_slice(),
                albums: vec![].into_boxed_slice(),
                runtime: Duration::from_secs(0),
            })
            .await
        }
    }

    #[instrument]
    pub async fn create_or_read_by_names(names: &[Arc<str>]) -> Result<Vec<Artist>, Error> {
        let mut artists = Vec::with_capacity(names.len());
        for name in names {
            if let Some(id) = Artist::create_or_read_by_name(name).await? {
                artists.push(id);
            }
        }
        Ok(artists)
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
        Ok(db()
            .await?
            .select(TABLE_NAME)
            .await?
            .into_iter()
            .filter(|x: &Artist| ids.contains(&x.id))
            .collect::<OneOrMany<_>>())
    }

    #[instrument]
    pub async fn update(id: ArtistId, artist: Artist) -> Result<(), Error> {
        let _: Artist = db()
            .await?
            .update((TABLE_NAME, id))
            .content(artist)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    #[instrument]
    pub async fn add_album(id: ArtistId, album_id: AlbumId) -> Result<(), Error> {
        let mut artist = Artist::read(id.clone()).await?.ok_or(Error::NotFound)?;
        let album = Album::read(album_id.clone())
            .await?
            .ok_or(Error::NotFound)?;

        artist.runtime += album.runtime;
        artist.songs = artist
            .songs
            .iter()
            .cloned()
            .chain(album.songs.iter().cloned())
            .collect();

        artist.albums = artist
            .albums
            .iter()
            .cloned()
            .chain(Some(album_id))
            .collect();

        let _: Artist = db()
            .await?
            .update((TABLE_NAME, id))
            .content(artist)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    #[instrument]
    pub async fn remove_songs(id: ArtistId, song_ids: &[SongId]) -> Result<(), Error> {
        let mut artist = Artist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        artist.songs = artist
            .songs
            .iter()
            .filter(|x| !song_ids.contains(x))
            .cloned()
            .collect();

        let _: Artist = db()
            .await?
            .update((TABLE_NAME, id))
            .content(artist)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    /// goes through all the songs in the artist and removes any that don't exist in the database
    /// also goes through the albums and removes any that don't exist in the database
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the artist to repair
    ///
    /// # Returns
    ///
    /// * `bool` - whether the artist was removed or not (if it has no songs or albums, it should be removed)
    #[instrument]
    pub async fn repair(id: ArtistId) -> Result<bool, Error> {
        let mut artist = Artist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        let mut new_songs = Vec::with_capacity(artist.songs.len());
        for song_id in artist.songs.iter() {
            if Song::read(song_id.clone()).await?.is_some() {
                new_songs.push(song_id.clone());
            }
        }

        artist.songs = new_songs.into_boxed_slice();

        let mut new_albums = Vec::with_capacity(artist.albums.len());
        for album_id in artist.albums.iter() {
            if Album::read(album_id.clone()).await?.is_some() {
                new_albums.push(album_id.clone());
            }
        }

        artist.albums = new_albums.into_boxed_slice();

        let result: Result<Artist, _> = db()
            .await?
            .update((TABLE_NAME, id.clone()))
            .content(artist)
            .await?
            .ok_or(Error::NotFound);

        if result.map(|x| x.songs.is_empty() && x.albums.is_empty())? {
            let _: Option<Artist> = db().await?.delete((TABLE_NAME, id)).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
