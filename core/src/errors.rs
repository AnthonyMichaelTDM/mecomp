use mecomp_storage::errors::Error;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Config error: {0}")]
    Config(String),
}

impl From<Error> for LibraryError {
    fn from(e: Error) -> Self {
        LibraryError::Database(e.to_string())
    }
}
