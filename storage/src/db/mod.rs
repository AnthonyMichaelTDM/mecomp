#[cfg(feature = "db")]
pub mod crud;
#[cfg(feature = "db")]
pub mod health;
#[cfg(feature = "db")]
pub mod queries;
pub mod schemas;

#[cfg(feature = "db")]
use surrealdb::{
    engine::local::{Db, SurrealKV},
    Surreal,
};

#[cfg(feature = "db")]
static DB_DIR: once_cell::sync::OnceCell<std::path::PathBuf> = once_cell::sync::OnceCell::new();
#[cfg(feature = "db")]
static TEMP_DB_DIR: once_cell::sync::Lazy<tempfile::TempDir> = once_cell::sync::Lazy::new(|| {
    tempfile::tempdir().expect("Failed to create temporary directory")
});

/// NOTE: if you change this, you must go through the schemas and update the index analyzer names
pub const FULL_TEXT_SEARCH_ANALYZER_NAME: &str = "custom_analyzer";

/// Set the path to the database.
///
/// # Errors
///
/// This function will return an error if the path cannot be set.
#[cfg(feature = "db")]
pub fn set_database_path(path: std::path::PathBuf) -> Result<(), crate::errors::Error> {
    DB_DIR
        .set(path)
        .map_err(crate::errors::Error::DbPathSetError)?;
    log::info!("Primed database path");
    Ok(())
}

/// Initialize the database with the necessary tables.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
#[cfg(feature = "db")]
pub async fn init_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<SurrealKV>(DB_DIR
        .get().cloned()
        .unwrap_or_else(|| {
            log::warn!("DB_DIR not set, defaulting to a temporary directory `{}`, this is likely a bug because `init_database` should be called before `db`", TEMP_DB_DIR.path().display());
            TEMP_DB_DIR.path()
            .to_path_buf()
        })).await?;

    db.use_ns("mecomp").use_db("music").await?;

    register_custom_analyzer(&db).await?;
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

#[cfg(feature = "db")]
pub(crate) async fn register_custom_analyzer<C>(db: &Surreal<C>) -> surrealdb::Result<()>
where
    C: surrealdb::Connection,
{
    use queries::define_analyzer;
    use surrealdb::sql::Tokenizer;

    db.query(define_analyzer(
        FULL_TEXT_SEARCH_ANALYZER_NAME,
        Some(Tokenizer::Class),
        &["snowball(english)"],
    ))
    .await?;

    Ok(())
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

        // register the custom analyzer
        register_custom_analyzer(&db).await?;

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
