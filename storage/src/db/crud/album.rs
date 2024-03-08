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

    pub async fn remove_song(&self, song_id: SongId) -> Result<(), Error> {
        let Some(album_id) = self.id.clone() else {
            return Err(Error::NoId);
        };

        let mut songs = self.songs.clone();
        songs.retain(|x| *x != song_id);
        let album = Album {
            songs,
            ..self.clone()
        };

        let result: Option<Album> = DB.update((TABLE_NAME, album_id)).content(album).await?;

        if result.is_none() {
            return Err(Error::NotFound);
        }

        Ok(())
    }
}
