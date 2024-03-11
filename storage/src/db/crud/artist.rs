//! CRUD operations for the artist table
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
    pub async fn create(artist: Artist) -> Result<Option<ArtistId>, Error> {
        let id = DB
            .create((TABLE_NAME, artist.id.clone()))
            .content(artist)
            .await?
            .map(|x: Artist| x.id);
        Ok(id)
    }

    pub async fn read_all() -> Result<Vec<Artist>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: ArtistId) -> Result<Option<Artist>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn read_by_name(name: &str) -> Result<Option<Artist>, Error> {
        Ok(DB.select((TABLE_NAME, name)).await?)
    }

    pub async fn update(id: ArtistId, artist: Artist) -> Result<(), Error> {
        let result = DB.update((TABLE_NAME, id)).content(artist).await?;
        result.ok_or(Error::NotFound)
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
