//! CRUD operations for the playlist table
use std::time::Duration;

use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::playlist::{add_songs, read_by_name, read_songs, remove_songs},
        schemas::{
            playlist::{Playlist, PlaylistChangeSet, PlaylistId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::Error,
};

impl Playlist {
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        playlist: Self,
    ) -> Result<Option<Self>, Error> {
        Ok(db
            .create((TABLE_NAME, playlist.id.clone()))
            .content(playlist)
            .await?)
    }

    #[instrument]
    pub async fn create_copy<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> Result<Option<Self>, Error> {
        // first we get the playlist we're copying
        let Some(playlist) = Self::read(db, id.clone()).await? else {
            return Ok(None);
        };

        // next we create a new playlist with the same name (with "copy" appended)
        let Some(new_playlist) = Self::create(
            db,
            Self {
                id: Self::generate_id(),
                name: format!("{} (copy)", playlist.name).into(),
                song_count: 0,
                runtime: Duration::from_secs(0),
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
            &Self::read_songs(db, id.clone())
                .await?
                .iter()
                .map(|song| song.id.clone())
                .collect::<Vec<_>>(),
        )
        .await?;

        Self::read(db, new_playlist.id.clone()).await
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Self>, Error> {
        Ok(db.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> Result<Option<Self>, Error> {
        Ok(db.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_by_name<C: Connection>(
        db: &Surreal<C>,
        name: String,
    ) -> Result<Option<Self>, Error> {
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
    ) -> Result<Option<Self>, Error> {
        Ok(db.update((TABLE_NAME, id)).merge(changes).await?)
    }

    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
    ) -> Result<Option<Self>, Error> {
        Ok(db.delete((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
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
        id: PlaylistId,
    ) -> Result<Vec<Song>, Error> {
        Ok(db.query(read_songs()).bind(("id", id)).await?.take(0)?)
    }

    #[instrument]
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db.query(remove_songs())
            .bind(("id", id.clone()))
            .bind(("songs", song_ids))
            .await?;
        Self::repair(db, id).await?;
        Ok(())
    }

    /// updates the song_count and runtime of the playlist
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the playlist to repair
    #[instrument]
    pub async fn repair<C: Connection>(db: &Surreal<C>, id: PlaylistId) -> Result<bool, Error> {
        let songs = Self::read_songs(db, id.clone()).await?;

        Self::update(
            db,
            id,
            PlaylistChangeSet {
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
        Playlist::add_songs(&db, playlist.id.clone(), &[song.id.clone()]).await?;
        // clone the playlist
        let result = Playlist::create_copy(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found after being cloned"))?;

        // ensure the playlist was cloned correctly
        assert_str_eq!(result.name, format!("{} (copy)", playlist.name).into());
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
        Ok(())
    }

    #[tokio::test]
    async fn test_read_by_name() -> Result<()> {
        let db = init_test_database().await?;
        let playlist = create_playlist();
        Playlist::create(&db, playlist.clone()).await?;
        let result = Playlist::read_by_name(&db, playlist.name.as_ref().to_string()).await?;
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
            ..Default::default()
        };

        let updated = Playlist::update(&db, playlist.id.clone(), changes).await?;
        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;

        assert_eq!(read.name, "Updated Name".into());
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

        Playlist::add_songs(&db, playlist.id.clone(), &[song.id.clone()]).await?;

        let result = Playlist::read_songs(&db, playlist.id.clone()).await?;
        assert_eq!(result, vec![song.clone()]);

        let read = Playlist::read(&db, playlist.id.clone())
            .await?
            .ok_or_else(|| anyhow!("Playlist not found"))?;
        assert_eq!(read.song_count, 1);
        assert_eq!(read.runtime, song.runtime);

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

        Playlist::add_songs(&db, playlist.id.clone(), &[song.id.clone()]).await?;
        Playlist::remove_songs(&db, playlist.id.clone(), &[song.id.clone()]).await?;

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
