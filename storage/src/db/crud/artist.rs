//! CRUD operations for the song table
use crate::{
    db::{
        schemas::{
            artist::{Artist, ArtistId, TABLE_NAME},
            song::SongId,
        },
        DB,
    },
    errors::Error,
};

impl Artist {
    pub async fn read_all() -> Result<Vec<Artist>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: ArtistId) -> Result<Option<Artist>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn remove_song(id: ArtistId, song_id: SongId) -> Result<(), Error> {
        let mut artist = Artist::read(id.clone()).await?.ok_or(Error::NotFound)?;

        artist.songs = artist
            .songs
            .iter()
            .filter(|x| **x != song_id)
            .cloned()
            .collect();

        DB.update((TABLE_NAME, id))
            .content(artist)
            .await?
            .ok_or(Error::NotFound)
    }
}
