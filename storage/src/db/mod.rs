#[cfg(any(test, feature = "db"))]
pub mod crud;
#[cfg(any(test, feature = "db"))]
pub mod health;
pub mod schemas;

#[cfg(any(test, feature = "db"))]
use surrealdb::{
    engine::local::{Db, Mem, SurrealKV},
    Surreal,
};

#[cfg(any(test, feature = "db"))]
static DB_DIR: tokio::sync::OnceCell<std::path::PathBuf> = tokio::sync::OnceCell::const_new();
#[cfg(any(test, feature = "db"))]
static TEMP_DB_DIR: once_cell::sync::Lazy<tempfile::TempDir> = once_cell::sync::Lazy::new(|| {
    tempfile::tempdir().expect("Failed to create temporary directory")
});

/// Set the path to the database.
///
/// # Errors
///
/// This function will return an error if the path cannot be set.
#[cfg(any(test, feature = "db"))]
pub fn set_database_path(
    path: std::path::PathBuf,
) -> Result<(), tokio::sync::SetError<std::path::PathBuf>> {
    DB_DIR.set(path)?;
    log::info!("Primed database path");
    Ok(())
}

/// Initialize the database with the necessary tables.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
#[cfg(any(test, feature = "db"))]
pub async fn init_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<SurrealKV>(DB_DIR
        .get().cloned()
        .unwrap_or_else(|| {
            log::warn!("DB_DIR not set, defaulting to a temporary directory `{}`, this is likely a bug because `init_database` should be called before `db`", TEMP_DB_DIR.path().display());
            TEMP_DB_DIR.path()
            .to_path_buf()
        })).await?;

    db.use_ns("mecomp").use_db("music").await?;

    surrealqlx::register_tables!(
        &db,
        schemas::album::Album,
        schemas::artist::Artist,
        schemas::song::Song,
        schemas::collection::Collection,
        schemas::playlist::Playlist
    )?;

    Ok(db)
}

/// Initialize a test database with the same tables as the main database.
/// This is useful for testing queries and mutations.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
#[cfg(any(test, feature = "db"))]
pub async fn init_test_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("test").await?;

    surrealqlx::register_tables!(
        &db,
        schemas::album::Album,
        schemas::artist::Artist,
        schemas::song::Song,
        schemas::collection::Collection,
        schemas::playlist::Playlist
    )?;

    Ok(db)
}

#[cfg(test)]
mod test {
    use super::schemas::{
        album::Album, artist::Artist, collection::Collection, playlist::Playlist, song::Song,
    };
    use super::*;

    use surrealdb::engine::local::Mem;
    use surrealqlx::traits::Table;

    #[tokio::test]
    async fn test_register_tables() -> anyhow::Result<()> {
        // use an in-memory db for testing
        let db = Surreal::new::<Mem>(()).await?;
        db.use_ns("test").use_db("test").await?;

        // first we init all the table to ensure that the queries made by the macro work without error
        <Album as Table>::init_table(&db).await?;
        <Artist as Table>::init_table(&db).await?;
        <Song as Table>::init_table(&db).await?;
        <Collection as Table>::init_table(&db).await?;
        <Playlist as Table>::init_table(&db).await?;
        // then we try initializing one of the tables again to ensure that initialization won't mess with existing tables/data
        <Album as Table>::init_table(&db).await?;

        Ok(())
    }
}
