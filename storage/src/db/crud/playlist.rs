//! CRUD operations for the playlist table
use crate::{
    db::{
        schemas::{
            playlist::{Playlist, PlaylistId, TABLE_NAME},
            song::SongId,
        },
        DB,
    },
    errors::Error,
};

impl Playlist {
    pub async fn read_all() -> Result<Vec<Playlist>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: PlaylistId) -> Result<Option<Playlist>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn remove_song(id: PlaylistId, song_id: SongId) -> Result<(), Error> {
        let mut playlist = Playlist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        playlist.songs = playlist
            .songs
            .iter()
            .filter(|x| **x != song_id)
            .cloned()
            .collect();

        DB.update((TABLE_NAME, id))
            .content(playlist)
            .await?
            .ok_or(Error::NotFound)
    }
}
