use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("SurrealDB error: {0}")]
    DbError(#[from] surrealdb::Error),
    #[error("Failed to set database path: {0}")]
    SetError(#[from] tokio::sync::SetError<PathBuf>),
    #[error("Item is missing an Id.")]
    NoId,
    #[error("Item not found.")]
    NotFound,
    #[error("Song IO error: {0}")]
    SongIOError(#[from] SongIOError),
    #[error("Item not created.")]
    NotCreated,
}

#[derive(Error, Debug)]
pub enum SongIOError {
    #[error("IO error: {0}")]
    FsError(#[from] std::io::Error),
    #[error("Audiotag error: {0}")]
    AudiotagError(#[from] audiotags::Error),
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    #[error("Duration not found")]
    DurationNotFound,
    #[error("Song already exists in the database")]
    SongExists,
    #[error("couldn't read duration from metadata")]
    DurationReadError,
}
