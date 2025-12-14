//! CRUD operations for the collection table
use std::time::Duration;

use surrealdb::{Connection, Surreal};
use surrealqlx::surrql;
use tracing::instrument;

use crate::{
    db::{
        queries::collection::{add_songs, read_songs, remove_songs},
        schemas::{
            collection::{
                Collection, CollectionBrief, CollectionChangeSet, CollectionId, TABLE_NAME,
            },
            playlist::Playlist,
            song::{Song, SongId},
        },
    },
    errors::{Error, StorageResult},
};

impl Collection {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        collection: Self,
    ) -> StorageResult<Option<Self>> {
        Ok(db.create(collection.id.clone()).content(collection).await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read_all_brief<C: Connection>(
        db: &Surreal<C>,
    ) -> StorageResult<Vec<CollectionBrief>> {
        Ok(db
            .query(surrql!(
                "SELECT type::fields($fields) FROM type::table($table)"
            ))
            .bind(("fields", Self::BRIEF_FIELDS))
            .bind(("table", TABLE_NAME))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.select(id).await?)
    }

    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        changes: CollectionChangeSet,
    ) -> StorageResult<Option<Self>> {
        Ok(db.update(id).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> StorageResult<Option<Self>> {
        // first remove all the songs from the collection
        let songs = Self::read_songs(db, id.clone())
            .await?
            .into_iter()
            .map(|song| song.id)
            .collect::<Vec<_>>();
        Self::remove_songs(db, id.clone(), songs).await?;

        Ok(db.delete(id).await?)
    }

    #[instrument]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        song_ids: Vec<SongId>,
    ) -> StorageResult<()> {
        db.query(add_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
    ) -> StorageResult<Vec<Song>> {
        Ok(db.query(read_songs()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    /// removes songs from a collection
    ///
    /// # Returns
    ///
    /// * `bool` - True if the collection is empty
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        song_ids: Vec<SongId>,
    ) -> StorageResult<()> {
        db.query(remove_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    /// Delete all orphaned collections
    ///
    /// An orphaned collection is a collection that has no songs in it
    pub async fn delete_orphaned<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db
            .query(surrql!(
                "DELETE FROM collection WHERE type::int(song_count) = 0 RETURN BEFORE"
            ))
            .await?
            .take(0)?)
    }

    /// "Freeze" a collection, this will create a playlist with the given name that contains all the songs in the given collection
    #[instrument]
    pub async fn freeze<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        name: String,
    ) -> StorageResult<Playlist> {
        // create the new playlist
        let playlist = Playlist::create(
            db,
            Playlist {
                id: Playlist::generate_id(),
                name,
                runtime: Duration::default(),
                song_count: 0,
            },
        )
        .await?
        .ok_or(Error::NotFound)?;

        // get the songs in the collection
        let songs = Self::read_songs(db, id.clone()).await?;
        let song_ids = songs.into_iter().map(|song| song.id).collect::<Vec<_>>();

        // add the songs to the playlist
        Playlist::add_songs(db, playlist.id.clone(), song_ids).await?;

        // get the playlist
        let playlist = Playlist::read(db, playlist.id.clone())
            .await?
            .ok_or(Error::NotFound)?;

        Ok(playlist)
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

    use anyhow::{Result, anyhow};
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
        assert_eq!(result, vec![collection.clone()]);

        let result = Collection::read_all_brief(&db).await?;
        assert!(!result.is_empty());
        assert_eq!(result, vec![collection.into()]);
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
        };

        let updated = Collection::update(&db, collection.id.clone(), changes).await?;
        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;

        assert_eq!(read.name, "Updated Name");
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

        Collection::add_songs(&db, collection.id.clone(), vec![song.id.clone()]).await?;

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

        Collection::add_songs(&db, collection.id.clone(), vec![song.id.clone()]).await?;
        let result = Collection::read_songs(&db, collection.id.clone()).await?;
        assert_eq!(result, vec![song.clone()]);

        Collection::remove_songs(&db, collection.id.clone(), vec![song.id.clone()]).await?;
        let result = Collection::read_songs(&db, collection.id.clone()).await?;
        assert_eq!(result, vec![]);

        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 0);
        assert_eq!(read.runtime, Duration::from_secs(0));

        Ok(())
    }

    #[tokio::test]
    async fn test_freeze() -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection();
        Collection::create(&db, collection.clone()).await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Collection::add_songs(&db, collection.id.clone(), vec![song.id.clone()]).await?;

        let playlist =
            Collection::freeze(&db, collection.id.clone(), "Frozen Playlist".into()).await?;

        let songs = Playlist::read_songs(&db, playlist.id.clone()).await?;

        assert_eq!(songs, vec![song.clone()]);
        assert_eq!(playlist.song_count, 1);
        assert_eq!(playlist.runtime, song.runtime);
        assert_eq!(playlist.name, "Frozen Playlist");

        Ok(())
    }
}
