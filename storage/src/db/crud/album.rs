//! CRUD operations for the song table
use crate::{
    db::{
        schemas::{
            album::{Album, AlbumId, TABLE_NAME},
            song::SongId,
        },
        DB,
    },
    errors::Error,
};

impl Album {
    pub async fn read_all() -> Result<Vec<Album>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: AlbumId) -> Result<Option<Album>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn remove_song(id: AlbumId, song_id: SongId) -> Result<(), Error> {
        let mut album = Album::read(id.clone()).await?.ok_or(Error::NotFound)?;

        album.songs = album
            .songs
            .iter()
            .filter(|x| **x != song_id)
            .cloned()
            .collect();

        let result = DB
            .update((TABLE_NAME, id))
            .content(album)
            .await?
            .ok_or(Error::NotFound);

        result
    }
}
