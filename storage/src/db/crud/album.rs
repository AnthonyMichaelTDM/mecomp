//! CRUD operations for the album table
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
    pub async fn create(album: Album) -> Result<Option<AlbumId>, Error> {
        let id = DB
            .create((TABLE_NAME, album.id.clone()))
            .content(album)
            .await?
            .map(|x: Album| x.id);
        Ok(id)
    }

    pub async fn read_all() -> Result<Vec<Album>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: AlbumId) -> Result<Option<Album>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn read_by_name(name: &str) -> Result<Vec<Album>, Error> {
        Ok(DB
            .select(TABLE_NAME)
            .await?
            .into_iter()
            .filter(|x: &Album| x.title.as_ref() == name)
            .collect::<Vec<_>>())
    }

    pub async fn update(id: AlbumId, album: Album) -> Result<(), Error> {
        let result = DB.update((TABLE_NAME, id)).content(album).await?;
        result.ok_or(Error::NotFound)
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
