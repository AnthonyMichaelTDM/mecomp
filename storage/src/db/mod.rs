pub mod crud;
pub mod schemas;

use std::path::PathBuf;

use log::info;
use once_cell::sync::Lazy;
use surrealdb::engine::local::Mem;
use surrealdb::{
    engine::local::{Db, SpeeDb},
    Surreal,
};
use surrealqlx::register_tables;
use tempfile::TempDir;
use tokio::sync::{OnceCell, SetError};

static DB_DIR: OnceCell<PathBuf> = OnceCell::const_new();
static TEMP_DB_DIR: Lazy<TempDir> =
    Lazy::new(|| tempfile::tempdir().expect("Failed to create temporary directory"));

pub async fn set_database_path(path: PathBuf) -> Result<(), SetError<PathBuf>> {
    DB_DIR.set(path)?;
    info!("Primed database path");
    Ok(())
}

pub async fn init_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<SpeeDb>(DB_DIR
        .get().cloned()
        .unwrap_or_else(|| {
            log::warn!("DB_DIR not set, defaulting to a temporary directory `{}`, this is likely a bug because `init_database` should be called before `db`", TEMP_DB_DIR.path().display());
            TEMP_DB_DIR.path()
            .to_path_buf()
        })).await?;

    db.use_ns("mecomp").use_db("music").await?;

    register_tables!(
        &db,
        schemas::album::Album,
        schemas::artist::Artist,
        schemas::song::Song,
        schemas::collection::Collection,
        schemas::playlist::Playlist
    )?;

    Ok(db)
}

pub async fn init_test_database() -> surrealdb::Result<Surreal<Db>> {
    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("test").await?;

    register_tables!(
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
