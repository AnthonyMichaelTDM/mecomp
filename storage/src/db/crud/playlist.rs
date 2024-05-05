//! CRUD operations for the playlist table
use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::schemas::{
        playlist::{Playlist, PlaylistId, TABLE_NAME},
        song::SongId,
    },
    errors::Error,
};

impl Playlist {
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
    pub async fn add_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db
            .query("RELATE $id->playlist_to_song->$songs")
            .query("UPDATE $id SET song_count += array::len($songs), runtime += math::sum(SELECT runtime FROM $songs)")
            .bind(("id", id))
            .bind(("songs", song_ids))
            .await?;
        Ok(())
    }

    #[instrument]
    pub async fn remove_songs<C: Connection>(
        db: &Surreal<C>,
        id: PlaylistId,
        song_ids: &[SongId],
    ) -> Result<(), Error> {
        db
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
    pub async fn repair<C: Connection>(db: &Surreal<C>, id: PlaylistId) -> Result<(), Error> {
        db
            .query("UPDATE $id SET song_count = array::len(SELECT ->playlist_to_song->song FROM ONLY $id), runtime = math::sum(SELECT runtime FROM (SELECT ->playlist_to_song->song FROM ONLY $id))")
            .bind(("id", id))
            .await?;
        Ok(())
    }
}
