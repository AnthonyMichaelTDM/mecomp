//! CRUD operations for the playlist table
use tracing::instrument;

use crate::{
    db::{
        db,
        schemas::{
            playlist::{Playlist, PlaylistId, TABLE_NAME},
            song::SongId,
        },
    },
    errors::Error,
};

impl Playlist {
    #[instrument]
    pub async fn read_all() -> Result<Vec<Playlist>, Error> {
        Ok(db().await.select(TABLE_NAME).await?)
    }

    #[instrument]
    pub async fn read(id: PlaylistId) -> Result<Option<Playlist>, Error> {
        Ok(db().await.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn add_songs(id: PlaylistId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await
            .query("RELATE $id->playlist_to_song->$songs")
            .query("UPDATE $id SET song_count += array::len($songs), runtime += math::sum(SELECT runtime FROM $songs)")
            .bind(("id", id))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn remove_songs(id: PlaylistId, song_ids: &[SongId]) -> Result<(), Error> {
        db().await
            .query("UNRELATE $id->playlist_to_song->$songs")
            .query("UPDATE $id SET song_count -= array::len($songs), runtime -= math::sum(SELECT runtime FROM $songs)")
            .bind(("id", id))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    /// updates the song_count and runtime of the playlist
    ///
    /// # Arguments
    ///
    /// * `id` - the id of the playlist to repair
    #[instrument]
    pub async fn repair(id: PlaylistId) -> Result<(), Error> {
        db().await
            .query("UPDATE $id SET song_count = array::len(SELECT ->playlist_to_song->song FROM ONLY $id), runtime = math::sum(SELECT runtime FROM (SELECT ->playlist_to_song->song FROM ONLY $id))")
            .bind(("id", id))
            .await?;
        Ok(())
    }
}
