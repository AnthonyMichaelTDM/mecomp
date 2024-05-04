//! CRUD operations for the collection table
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            collection::{Collection, CollectionChangeSet, CollectionId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::Error,
};

impl Collection {
    #[instrument]
    pub async fn create(collection: Collection) -> Result<Option<Collection>, Error> {
        Ok(db()
            .await
            .create((TABLE_NAME, collection.id.clone()))
            .content(collection)
            .await?)
    }

    #[instrument]
    pub async fn read_all() -> Result<Vec<Collection>, Error> {
        Ok(db().await.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: CollectionId) -> Result<Option<Collection>, Error> {
        Ok(db().await.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn update(
        id: CollectionId,
        changes: CollectionChangeSet,
    ) -> Result<Option<Collection>, Error> {
        Ok(db().await.update((TABLE_NAME, id)).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete(id: CollectionId) -> Result<Option<Collection>, Error> {
        Ok(db().await.delete((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs(id: CollectionId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await
            .query("RELATE $id->collection_to_song->$songs")
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Collection::repair(id).await?;
        Ok(())
    }

    #[instrument]
    pub async fn read_songs(id: CollectionId) -> Result<Vec<Song>, Error> {
        Ok(db()
            .await
            .query("SELECT * FROM $id->collection_to_song->song")
            .bind(("id", id))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn remove_songs(id: CollectionId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await
            .query("DELETE $id->collection_to_song WHERE out IN $songs")
            .query("UPDATE $id SET song_count -= array::len($songs), runtime-=math::sum((SELECT runtime FROM $song))")
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Collection::repair(id).await?;
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
    pub async fn repair(id: CollectionId) -> Result<bool, Error> {
        let songs = Collection::read_songs(id.clone()).await?;

        db().await
            .query("UPDATE $id SET song_count=$songs, runtime=$runtime")
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
    use crate::{test_utils::ulid, util::OneOrMany};

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
        let collection = create_collection(&ulid);
        let result = Collection::create(collection.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_all(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
        let result = Collection::read_all().await?;
        assert!(result.len() > 0);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
        let result = Collection::read(collection.id.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_update(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
        let changes = CollectionChangeSet {
            name: Some(format!("Updated Name {ulid}").into()),
        };

        let updated = Collection::update(collection.id.clone(), changes).await?;
        let read = Collection::read(collection.id.clone())
            .await?
            .ok_or(anyhow!("Collection not found"))?;

        assert_eq!(read.name, format!("Updated Name {ulid}").into());
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_delete(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
        let result = Collection::delete(collection.id.clone()).await?;
        assert_eq!(result, Some(collection));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_add_songs(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
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
        Song::create(song.clone()).await?;

        Collection::add_songs(collection.id.clone(), &[song.id.clone()]).await?;

        let result = Collection::read_songs(collection.id.clone()).await?;
        assert_eq!(result, vec![song]);

        let read = Collection::read(collection.id.clone())
            .await?
            .ok_or(anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, Duration::from_secs(5));

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_remove_songs(ulid: String) -> Result<()> {
        let collection = create_collection(&ulid);
        Collection::create(collection.clone()).await?;
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
        Song::create(song.clone()).await?;

        Collection::add_songs(collection.id.clone(), &[song.id.clone()]).await?;
        Collection::remove_songs(collection.id.clone(), &[song.id.clone()]).await?;

        let result = Collection::read_songs(collection.id.clone()).await?;
        assert_eq!(result, vec![]);

        let read = Collection::read(collection.id.clone())
            .await?
            .ok_or(anyhow!("Collection not found"))?;
        assert_eq!(read.song_count, 0);
        assert_eq!(read.runtime, Duration::from_secs(0));

        Ok(())
    }
}
