//! CRUD operations for the song table
use crate::db::{errors::DatabaseError, schemas::song, DB};

use super::super::schemas::song::Song;

impl Song {
    pub async fn read_all() -> Result<Vec<Song>, DatabaseError> {
        Ok(DB.select(song::TABLE_NAME).await?)
    }

    pub async fn delete(id: song::SongId) -> Result<(), DatabaseError> {
        DB.delete((song::TABLE_NAME, id)).await?;
        Ok(())
    }
}
