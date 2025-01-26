//! CRUD operations for dynamic playlists.

use surrealdb::{Connection, RecordId, Surreal};
use tracing::instrument;

use crate::{
    db::schemas::{
        dynamic::{DynamicPlaylist, DynamicPlaylistChangeSet, DynamicPlaylistId, TABLE_NAME},
        song::Song,
    },
    errors::StorageResult,
};

impl DynamicPlaylist {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        dynamic_playlist: Self,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .create(RecordId::from_inner(dynamic_playlist.id.clone()))
            .content(dynamic_playlist)
            .await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: DynamicPlaylistId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.select(RecordId::from_inner(id)).await?)
    }

    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: DynamicPlaylistId,
        change_set: DynamicPlaylistChangeSet,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .update(RecordId::from_inner(id))
            .merge(change_set)
            .await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: DynamicPlaylistId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.delete(RecordId::from_inner(id)).await?)
    }

    #[instrument]
    /// Gets the songs matching the DynamicPlaylist's query.
    pub async fn run_query<C: Connection>(&self, db: &Surreal<C>) -> StorageResult<Vec<Song>> {
        Ok(db.query(self.get_query()).await?.take(0)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::schemas::{dynamic::query::Query, song::SongChangeSet},
        test_utils::{arb_song_case, create_song_with_overrides, init_test_database},
    };

    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use std::str::FromStr as _;

    #[tokio::test]
    async fn test_crud() -> Result<()> {
        let db = init_test_database().await?;

        let song = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                title: Some("test song".into()),
                ..SongChangeSet::default()
            },
        )
        .await?;
        let dynamic_playlist = DynamicPlaylist {
            id: DynamicPlaylist::generate_id(),
            name: "test".into(),
            query: Query::from_str("title = \"a song\"")?,
        };

        let id = dynamic_playlist.id.clone();

        // Create
        let created = DynamicPlaylist::create(&db, dynamic_playlist.clone()).await?;
        assert_eq!(created, Some(dynamic_playlist.clone()));

        // Read
        let read = DynamicPlaylist::read(&db, id.clone()).await?;
        assert_eq!(read, Some(dynamic_playlist.clone()));

        // read all
        let all = DynamicPlaylist::read_all(&db).await?;
        assert_eq!(all, vec![dynamic_playlist.clone()]);

        // run query
        let songs = read.unwrap().run_query(&db).await?;
        assert_eq!(songs, vec![]);

        // Update
        let change_set = DynamicPlaylistChangeSet {
            query: Some(Query::from_str("title = \"test song\"")?), // Change the query to match the song
            ..Default::default()
        };
        let updated = DynamicPlaylist::update(&db, id.clone(), change_set).await?;
        assert_eq!(
            updated,
            Some(DynamicPlaylist {
                query: Query::from_str("title = \"test song\"")?,
                ..dynamic_playlist.clone()
            })
        );

        // run query
        let songs = updated.unwrap().run_query(&db).await?;
        assert_eq!(songs, vec![song]);

        // Delete
        let deleted = DynamicPlaylist::delete(&db, id.clone()).await?;
        assert_eq!(deleted, Some(dynamic_playlist.clone()));

        // read all
        let all = DynamicPlaylist::read_all(&db).await?;
        assert_eq!(all, vec![]);

        Ok(())
    }
}
