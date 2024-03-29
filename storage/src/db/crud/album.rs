//! CRUD operations for the album table
use std::sync::Arc;

use log::warn;
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            album::{Album, AlbumId, TABLE_NAME},
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
    pub async fn read_or_create_by_name_and_album_artist(
        title: &str,
        album_artists: &[Arc<str>],
    ) -> Result<Option<AlbumId>, Error> {
        if let Some(album) = Album::read_all()
            .await?
            .iter()
            .filter(|x| x.title.as_ref() == title)
            .find(|x| x.artist.iter().all(|y| album_artists.contains(y)))
        {
            Ok(Some(album.id.clone()))
        } else {
            match Album::create(Album {
                id: Album::generate_id(),
                title: title.into(),
                artist: album_artists.iter().cloned().collect(),
                songs: vec![].into_boxed_slice(),
                runtime: 0.into(),
                artist_id: Artist::create_or_read_by_names(album_artists)
                    .await?
                    .into_iter()
                    .map(|x| x.id)
                    .collect(),
                release: None,
                song_count: 0,
                discs: 1,
                genre: OneOrMany::None,
            })
            .await?
            {
                Some(album) => {
                    // we created a new album under some artists, so we need to update those artists
                    for artist in album.artist_id.iter() {
                        Artist::add_album(artist.clone(), album.id.clone()).await?;
                    }
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
            .select(TABLE_NAME)
            .await?
            .into_iter()
            .filter(|x: &Album| x.title.as_ref() == name)
            .collect::<Vec<_>>())
    }

    #[instrument()]
    pub async fn update(id: AlbumId, album: Album) -> Result<(), Error> {
        let _: Album = db()
            .await?
            .update((TABLE_NAME, id))
            .content(album)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    #[instrument()]
    pub async fn add_songs(id: AlbumId, song_id: &[SongId]) -> Result<(), Error> {
        let mut album = Album::read(id.clone()).await?.ok_or(Error::NotFound)?;

        album.songs = album.songs.iter().chain(song_id.iter()).cloned().collect();

        let _: Album = db()
            .await?
            .update((TABLE_NAME, id))
            .content(album)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    #[instrument()]
    pub async fn remove_songs(id: AlbumId, song_ids: &[SongId]) -> Result<(), Error> {
        let mut album = Album::read(id.clone()).await?.ok_or(Error::NotFound)?;

        album.songs = album
            .songs
            .iter()
            .filter(|x| !song_ids.contains(x))
            .cloned()
            .collect();

        let _: Album = db()
            .await?
            .update((TABLE_NAME, id))
            .content(album)
            .await?
            .ok_or(Error::NotFound)?;
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
    #[instrument()]
    pub async fn repair(id: AlbumId) -> Result<bool, Error> {
        let mut album = Album::read(id.clone()).await?.ok_or(Error::NotFound)?;

        let mut new_songs = Vec::with_capacity(album.songs.len());
        for song_id in album.songs.iter() {
            if let Some(song) = Song::read(song_id.clone()).await? {
                match (song.album_id == id.clone(), song.album == album.title) {
                    (true, true) => new_songs.push(song_id.clone()),
                    (false, true) => {
                        warn!(
                            "Song {} has album_id {} that doesn't match the album id {}, but album title {} matches the song's album title {}",
                            song_id, song.album_id, id, album.title, song.album
                        );
                        // if song.album_artist_id == album.artist_id {
                        //     info!("Song's album name, and album artists, match the album's name and artist, updating song's album_id to match the album's id");
                        //     Song::update(
                        //         song_id.clone(),
                        //         Song {
                        //             album_id: id.clone(),
                        //             ..song
                        //         },
                        //     )
                        //     .await?;
                        // }
                    }
                    (true, false) => {
                        warn!(
                            "Song {} has album_id {} that matches the album id {} but album title {} doesn't match the song's album title {}",
                            song_id, song.album_id, id, album.title, song.album
                        );
                    }
                    (false, false) => (),
                }
            }
        }

        album.songs = new_songs.into_boxed_slice();

        let result: Result<Album, _> = db()
            .await?
            .update((TABLE_NAME, id.clone()))
            .content(album.clone())
            .await?
            .ok_or(Error::NotFound);

        if result.map(|x| x.songs.is_empty())? {
            let _: Option<Album> = db().await?.delete((TABLE_NAME, id)).await?;
            // repair the album artists
            for artist_id in album.artist_id.iter() {
                Artist::repair(artist_id.clone()).await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
