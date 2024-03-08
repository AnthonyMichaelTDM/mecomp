use mecomp_storage::db::errors::DatabaseError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Config error: {0}")]
    Config(String),
}

impl From<DatabaseError> for LibraryError {
    fn from(e: DatabaseError) -> Self {
        LibraryError::Database(e.to_string())
    }
}
