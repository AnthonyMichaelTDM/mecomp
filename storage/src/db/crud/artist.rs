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

    pub async fn remove_song(&self, song_id: SongId) -> Result<(), Error> {
        let Some(artist_id) = self.id.clone() else {
            return Err(Error::NoId);
        };

        let mut songs = self.songs.clone();
        songs.retain(|x| *x != song_id);
        let artist = Artist {
            songs,
            ..self.clone()
        };

        let result: Option<Artist> = DB.update((TABLE_NAME, artist_id)).content(artist).await?;

        if result.is_none() {
            return Err(Error::NotFound);
        }

        Ok(())
    }
}
