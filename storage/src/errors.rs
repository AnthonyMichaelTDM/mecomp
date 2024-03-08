use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("SurrealDB error: {0}")]
    DbError(#[from] surrealdb::Error),
    #[error("Item is missing an Id.")]
    NoId,
    #[error("Item not found.")]
    NotFound,
}
