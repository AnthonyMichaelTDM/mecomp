//! CRUD operations for the collection table
use crate::{
    db::{
        schemas::{
            collection::{Collection, CollectionId, TABLE_NAME},
            song::SongId,
        },
        DB,
    },
    errors::Error,
};

impl Collection {
    pub async fn read_all() -> Result<Vec<Collection>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    pub async fn read(id: CollectionId) -> Result<Option<Collection>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    pub async fn remove_song(id: CollectionId, song_id: SongId) -> Result<(), Error> {
        let mut collection = Collection::read(id.clone()).await?.ok_or(Error::NotFound)?;

        collection.songs = collection
            .songs
            .iter()
            .filter(|x| **x != song_id)
            .cloned()
            .collect();

        DB.update((TABLE_NAME, id))
            .content(collection)
            .await?
            .ok_or(Error::NotFound)
    }
}
