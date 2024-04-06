//! CRUD operations for the collection table
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            collection::{Collection, CollectionId, TABLE_NAME},
            song::SongId,
        },
    },
    errors::Error,
};

impl Collection {
    #[instrument]
    pub async fn read_all() -> Result<Vec<Collection>, Error> {
        Ok(db().await?.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: CollectionId) -> Result<Option<Collection>, Error> {
        Ok(db().await?.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs(id: CollectionId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await?
            .query("RELATE $id->collection_to_song->$songs")
            .query("UPDATE $id SET song_count += array::len($songs)")
            .bind(("id", id))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn remove_songs(id: CollectionId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await?
            .query("DELETE $id->collection_to_song WHERE out IN $songs")
            .query("UPDATE $id SET song_count -= array::len($songs), runtime-=math::sum((SELECT duration FROM $song))")
            .bind(("id", id))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    /// updates the song_count and runtime of the collection
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the collection to repair
    #[instrument]
    pub async fn repair(id: CollectionId) -> Result<(), Error> {
        db().await?
            .query("UPDATE $id SET song_count = array::len(SELECT ->collection_to_song FROM ONLY $id), runtime = math::sum(SELECT runtime FROM (SELECT ->collection_to_song FROM ONLY $id))")
            .bind(("id", id))
            .await?;
        Ok(())
    }
}
