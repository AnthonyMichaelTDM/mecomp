use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::schemas::song::SongId;

#[derive(Error, Debug, Serialize)]
pub enum DatabaseError {
    #[error("Song table error: {0}")]
    Song(#[from] surrealdb::Error),
    #[error("Artist table  error: {0}")]
    Artist(ArtistError),
    #[error("Album table  error: {0}")]
    Album(AlbumError),
}

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum ArtistError {
    #[error("The artist {0} already exists in the table.")]
    ArtistExists(String),
    #[error("The artist with id {0} does not exist.")]
    ArtistNotFound(SongId),
}

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum AlbumError {
    #[error("The album {0} already exists in the table.")]
    AlbumExists(String),
    #[error("The album with id {0} does not exist.")]
    AlbumNotFound(SongId),
}
