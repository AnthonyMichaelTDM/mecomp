//! CRUD operations for the collection table
use std::time::Duration;

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::collection::{add_songs, read_songs, remove_songs},
        schemas::{
            collection::{Collection, CollectionChangeSet, CollectionId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::Error,
};

impl Collection {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        collection: Self,
    ) -> Result<Option<Self>, Error> {
        Ok(db
            .create((TABLE_NAME, collection.id.clone()))
            .content(collection)
            .await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Self>, Error> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> Result<Option<Self>, Error> {
        Ok(db.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        changes: CollectionChangeSet,
    ) -> Result<Option<Self>, Error> {
        Ok(db.update((TABLE_NAME, id)).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> Result<Option<Self>, Error> {
        Ok(db.delete((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(add_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Self::repair(db, id).await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> Result<Vec<Song>, Error> {
        Ok(db.query(read_songs()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(remove_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Self::repair(db, id).await?;
        Ok(())
    }

    /// updates the song_count and runtime of the collection
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the collection to repair
    ///
    /// # Returns
    ///
    /// * `bool` - True if the collection is empty
    #[instrument]
    pub async fn repair<C: Connection>(db: &Surreal<C>, id: CollectionId) -> Result<bool, Error> {
        let songs = Self::read_songs(db, id.clone()).await?;

        Self::update(
            db,
            id,
            CollectionChangeSet {
                song_count: Some(songs.len()),
                runtime: Some(songs.iter().map(|song| song.runtime).sum::<Duration>()),
                ..Default::default()
            },
        )
        .await?;

        Ok(songs.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        db::schemas::song::SongChangeSet,
        test_utils::{arb_song_case, create_song_with_overrides, init_test_database},
    };

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;

    fn create_collection() -> Collection {
        Collection {
            id: Collection::generate_id(),
            name: "Test Collection".into(),
            runtime: Duration::from_secs(0),
            song_count: 0,
        }
    }

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        let result = Collection::create(&db, collection.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::read_all(&db).await?;
        assert!(!result.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::read(&db, collection.id.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[tokio::test]
    async fn test_update() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let changes = CollectionChangeSet {
            name: Some("Updated Name".into()),
            ..Default::default()
        };

        let updated = Collection::update(&db, collection.id.clone(), changes).await?;
        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;

        assert_eq!(read.name, "Updated Name".into());
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::delete(&db, collection.id.clone()).await?;
        assert_eq!(result, Some(collection.clone()));
        let result = Collection::read(&db, collection.id).await?;
        assert_eq!(result, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_songs() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Collection::add_songs(&db, collection.id.clone(), &[song.id.clone()]).await?;

        let result = Collection::read_songs(&db, collection.id.clone()).await?;
        assert_eq!(result, vec![song.clone()]);

        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, song.runtime);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_songs() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Collection::add_songs(&db, collection.id.clone(), &[song.id.clone()]).await?;
        Collection::remove_songs(&db, collection.id.clone(), &[song.id.clone()]).await?;

        let result = Collection::read_songs(&db, collection.id.clone()).await?;
        assert_eq!(result, vec![]);

        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 0);
        assert_eq!(read.runtime, Duration::from_secs(0));

        Ok(())
    }
}
