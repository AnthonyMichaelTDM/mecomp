//! CRUD operations for the playlist table

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::playlist::{add_songs, read_by_name, read_songs, remove_songs},
        schemas::{
            playlist::{Playlist, PlaylistBrief, PlaylistChangeSet, PlaylistId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::StorageResult,
};

impl Playlist {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        playlist: Self,
    ) -> StorageResult<Option<Self>> {
        Ok(db.create(playlist.id.clone()).content(playlist).await?)
    }

    #[instrument]
    pub async fn create_copy<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> StorageResult<Option<Self>> {
        // first we get the playlist we're copying
        let Some(playlist) = Self::read(db, id.clone()).await? else {
            return Ok(None);
        };

        // next we create a new playlist with the same name (with "copy" appended)
        let Some(new_playlist) = Self::create(
            db,
            Self {
                id: Self::generate_id(),
                name: format!("{} (copy)", playlist.name),
                ..playlist
            },
        )
        .await?
        else {
            return Ok(None);
        };

        // then we add all the songs in the original playlist to the new playlist
        Self::add_songs(
            db,
            new_playlist.id.clone(),
            Self::read_songs(db, id.clone())
                .await?
                .iter()
                .map(|song| song.id.clone())
                .collect::<Vec<_>>(),
        )
        .await?;

        Self::read(db, new_playlist.id.clone()).await
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read_all_brief<C: Connection>(
        db: &Surreal<C>,
    ) -> StorageResult<Vec<PlaylistBrief>> {
        Ok(db.query("SELECT id,name FROM playlist;").await?.take(0)?)
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.select(id).await?)
    }

    #[instrument]
    pub async fn read_by_name<C: Connection>(
        db: &Surreal<C>,
        name: String,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .query(read_by_name())
            .bind(("name", name))
            .await?
            .take(0)?)
    }

    #[instrument]
    pub async fn update<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
        changes: PlaylistChangeSet,
    ) -> StorageResult<Option<Self>> {
        Ok(db.update(id).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.delete(id).await?)
    }

    #[instrument]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
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
        id: PlaylistId,
    ) -> StorageResult<Vec<Song>> {
        Ok(db.query(read_songs()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    /// removes songs from a playlist
    ///
    /// unlike the `remove_songs` methods for other tables,
    /// this method does not return whether the playlist is empty because
    /// having an empty playlist is a valid state that doesn't need to be checked
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
        song_ids: Vec<SongId>,
    ) -> StorageResult<()> {
        db.query(remove_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    /// Deletes all orphaned playlists from the database
    ///
    /// An orphaned playlist is a playlist that has no songs in it
    pub async fn delete_orphaned<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db
            .query("DELETE FROM playlist WHERE type::int(song_count) = 0 RETURN BEFORE")
            .await?
            .take(0)?)
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
    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::rstest;

    fn create_playlist() -> Playlist {
        Playlist {
            id: Playlist::generate_id(),
            name: "Test Playlist".into(),
            song_count: 0,
            runtime: Duration::from_secs(0),
        }
    }

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        let result = Playlist::create(&db, playlist.clone()).await?;
        assert_eq!(result, Some(playlist));
        Ok(())
    }

    #[tokio::test]
    async fn test_create_copy() -> Result<()> {
        let db = init_test_database().await?;
        // create playlist
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        // add a song to that playlist
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        Playlist::add_songs(&db, playlist.id.clone(), vec![song.id.clone()]).await?;
        // clone the playlist
        let result = Playlist::create_copy(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found after being cloned"))?;

        // ensure the playlist was cloned correctly
        assert_str_eq!(result.name, playlist.name + " (copy)");
        assert_eq!(result.song_count, 1);
        assert_eq!(result.runtime, song.runtime);

        assert_eq!(
            Playlist::read_songs(&db, result.id.clone()).await?,
            vec![song]
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let result = Playlist::read_all(&db).await?;
        assert!(!result.is_empty());
        assert_eq!(result, vec![playlist.clone()]);

        let result = Playlist::read_all_brief(&db).await?;
        assert!(!result.is_empty());
        assert_eq!(result, vec![playlist.into()]);
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_name() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let result = Playlist::read_by_name(&db, playlist.name.clone()).await?;
        assert_eq!(result, Some(playlist));
        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let result = Playlist::read(&db, playlist.id.clone()).await?;
        assert_eq!(result, Some(playlist));
        Ok(())
    }

    #[tokio::test]
    async fn test_update() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let changes = PlaylistChangeSet {
            name: Some("Updated Name".into()),
        };

        let updated = Playlist::update(&db, playlist.id.clone(), changes).await?;
        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;

        assert_eq!(read.name, "Updated Name");
        assert_eq!(Some(read), updated);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let result = Playlist::delete(&db, playlist.id.clone()).await?;
        assert_eq!(result, Some(playlist.clone()));
        let result = Playlist::read(&db, playlist.id).await?;
        assert_eq!(result, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_songs() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Playlist::add_songs(&db, playlist.id.clone(), vec![song.id.clone()]).await?;

        let result = Playlist::read_songs(&db, playlist.id.clone()).await?;
        assert_eq!(result, vec![song.clone()]);

        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, song.runtime);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_duplicate_songs() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Playlist::add_songs(&db, playlist.id.clone(), vec![song1.id.clone()]).await?;
        Playlist::add_songs(&db, playlist.id.clone(), vec![song1.id.clone()]).await?;
        Playlist::add_songs(
            &db,
            playlist.id.clone(),
            vec![song1.id.clone(), song1.id.clone(), song2.id.clone()],
        )
        .await?;

        let result = Playlist::read_songs(&db, playlist.id.clone()).await?;
        assert_eq!(result.len(), 2);
        assert!(
            result.contains(&song1),
            "Playlist should contain song1, but it doesn't: {result:?}"
        );
        assert!(
            result.contains(&song2),
            "Playlist should contain song2, but it doesn't: {result:?}"
        );

        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;
        assert_eq!(read.song_count, 2);
        assert_eq!(read.runtime, song1.runtime + song2.runtime);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_remove_songs() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        Playlist::add_songs(&db, playlist.id.clone(), vec![song.id.clone()]).await?;
        Playlist::remove_songs(&db, playlist.id.clone(), vec![song.id.clone()]).await?;

        let result = Playlist::read_songs(&db, playlist.id.clone()).await?;
        assert_eq!(result, vec![]);

        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;
        assert_eq!(read.song_count, 0);
        assert_eq!(read.runtime, Duration::from_secs(0));

        Ok(())
    }
}
