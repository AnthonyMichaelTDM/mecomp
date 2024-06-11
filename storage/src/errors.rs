use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[cfg(feature = "db")]
    #[error("SurrealDB error: {0}")]
    DbError(#[from] surrealdb::Error),
    #[error("Failed to set database path to {0}")]
    DbPathSetError(PathBuf),
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
    #[error("Lofty error: {0}")]
    LoftyError(#[from] lofty::error::LoftyError),
    #[error("Song missing audio tags")]
    MissingTags,
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    #[error("Song already exists in the database")]
    SongExists,
    #[error("couldn't read duration from metadata")]
    DurationReadError,
}
