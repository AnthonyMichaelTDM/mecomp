//! CRUD operations for the collection table
use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::schemas::{
        collection::{Collection, CollectionChangeSet, CollectionId, TABLE_NAME},
        song::{Song, SongId},
    },
    errors::Error,
};

use super::queries::collection::{
    read_songs_in_collection, relate_collection_to_songs, remove_songs_from_collection, repair,
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
        db.query(relate_collection_to_songs())
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
        Ok(db
            .query(read_songs_in_collection())
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: CollectionId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(remove_songs_from_collection())
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

        db.query(repair())
            .bind(("id", id))
            .bind(("songs", songs.len()))
            .bind((
                "runtime",
                songs
                    .iter()
                    .map(|song| song.runtime)
                    .sum::<surrealdb::sql::Duration>(),
            ))
            .await?;

        Ok(songs.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{db::init_test_database, test_utils::ulid, util::OneOrMany};

    use anyhow::{anyhow, Result};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use surrealdb::sql::Duration;

    fn create_collection(ulid: &str) -> Collection {
        Collection {
            id: Collection::generate_id(),
            name: format!("Test Collection {ulid}").into(),
            runtime: Duration::from_secs(0),
            song_count: 0,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_create(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        let result = Collection::create(&db, collection.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_all(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::read_all(&db).await?;
        assert!(!result.is_empty());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::read(&db, collection.id.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let changes = CollectionChangeSet {
            name: Some(format!("Updated Name {ulid}").into()),
        };

        let updated = Collection::update(&db, collection.id.clone(), changes).await?;
        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;

        assert_eq!(read.name, format!("Updated Name {ulid}").into());
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_delete(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let result = Collection::delete(&db, collection.id.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_add_songs(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(format!("Test Album {ulid}").into()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(format!("Test Album {ulid}").into()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };
        Song::create(&db, song.clone()).await?;

        Collection::add_songs(&db, collection.id.clone(), &[song.id.clone()]).await?;

        let result = Collection::read_songs(&db, collection.id.clone()).await?;
        assert_eq!(result, vec![song]);

        let read = Collection::read(&db, collection.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, Duration::from_secs(5));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_remove_songs(ulid: String) -> Result<()> {
        let db = init_test_database().await?;
        let collection = create_collection(&ulid);
        Collection::create(&db, collection.clone()).await?;
        let song = Song {
            id: Song::generate_id(),
            title: format!("Test Song {ulid}").into(),
            artist: OneOrMany::One(format!("Test Album {ulid}").into()),
            album: format!("Test Album {ulid}").into(),
            runtime: Duration::from_secs(5),
            track: Some(1),
            disc: Some(1),
            genre: OneOrMany::None,
            album_artist: OneOrMany::One(format!("Test Album {ulid}").into()),
            release_year: None,
            extension: "mp3".into(),
            path: PathBuf::from(format!("song_1_{}_{ulid}", rand::random::<usize>())),
        };
        Song::create(&db, song.clone()).await?;

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
