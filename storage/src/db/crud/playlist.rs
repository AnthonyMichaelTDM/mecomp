//! CRUD operations for the playlist table
use tracing::instrument;

use crate::{
    db::{
        schemas::{
            playlist::{Playlist, PlaylistId, TABLE_NAME},
            song::{Song, SongId},
        },
        DB,
    },
    errors::Error,
};

impl Playlist {
    #[instrument]
    pub async fn read_all() -> Result<Vec<Playlist>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: PlaylistId) -> Result<Option<Playlist>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs(id: PlaylistId, song_ids: &[SongId]) -> Result<(), Error> {
        let mut playlist = Playlist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        playlist.songs = playlist
            .songs
            .iter()
            .chain(song_ids.iter())
            .cloned()
            .collect();

        DB.update((TABLE_NAME, id))
            .content(playlist)
            .await?
            .ok_or(Error::NotFound)
    }

    #[instrument]
    pub async fn remove_songs(id: PlaylistId, song_ids: &[SongId]) -> Result<(), Error> {
        let mut playlist = Playlist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        playlist.songs = playlist
            .songs
            .iter()
            .filter(|x| !song_ids.contains(x))
            .cloned()
            .collect();

        let _: Playlist = DB
            .update((TABLE_NAME, id))
            .content(playlist)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    /// goes through all the songs in the playlist and removes any that don't exist in the database
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the playlist to repair
    ///
    /// # Returns
    ///
    /// true if the playlist is empty after the repair, false otherwise
    #[instrument]
    pub async fn repair(id: PlaylistId) -> Result<bool, Error> {
        let mut playlist = Playlist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        let mut new_songs = Vec::with_capacity(playlist.songs.len());
        for song_id in playlist.songs.iter() {
            if Song::read(song_id.clone()).await?.is_some() {
                new_songs.push(song_id.clone());
            }
        }

        playlist.songs = new_songs.into_boxed_slice();

        let result: Result<Playlist, _> = DB
            .update((TABLE_NAME, id))
            .content(playlist)
            .await?
            .ok_or(Error::NotFound);

        result.map(|x| x.songs.is_empty())
    }
}
