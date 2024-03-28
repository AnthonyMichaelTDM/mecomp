//! CRUD operations for the collection table
use tracing::instrument;

use crate::{
    db::{
        schemas::{
            collection::{Collection, CollectionId, TABLE_NAME},
            song::{Song, SongId},
        },
        DB,
    },
    errors::Error,
};

impl Collection {
    #[instrument]
    pub async fn read_all() -> Result<Vec<Collection>, Error> {
        Ok(DB.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: CollectionId) -> Result<Option<Collection>, Error> {
        Ok(DB.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn remove_songs(id: CollectionId, song_ids: &[SongId]) -> Result<(), Error> {
        let mut collection = Collection::read(id.clone()).await?.ok_or(Error::NotFound)?;

        collection.songs = collection
            .songs
            .iter()
            .filter(|x| !song_ids.contains(x))
            .cloned()
            .collect();

        let _: Collection = DB
            .update((TABLE_NAME, id))
            .content(collection)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    /// goes through all the songs in the collection and removes any that don't exist in the database
    /// TODO: Maybe in the future, we will want to recluster the collections in cases of a song being removed?
    /// but that can probably be another function
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the collection to repair
    ///
    /// # Returns
    ///
    /// * `bool` - Whether the collection was repaired or not
    ///
    #[instrument]
    pub async fn repair(id: CollectionId) -> Result<bool, Error> {
        let mut collection = Collection::read(id.clone()).await?.ok_or(Error::NotFound)?;

        let mut new_songs = Vec::with_capacity(collection.songs.len());
        for song_id in collection.songs.iter() {
            if Song::read(song_id.clone()).await?.is_some() {
                new_songs.push(song_id.clone());
            }
        }

        collection.songs = new_songs.into_boxed_slice();

        let result: Result<Collection, _> = DB
            .update((TABLE_NAME, id))
            .content(collection)
            .await?
            .ok_or(Error::NotFound);

        result.map(|x| x.songs.is_empty())
    }
}
