pub mod crud;
pub mod schemas;

use std::{ops::Deref, path::PathBuf};

use log::info;
use once_cell::sync::Lazy;
#[cfg(test)]
use surrealdb::engine::local::Mem;
use surrealdb::{engine::local::Db, Surreal};
use surrealqlx::register_tables;
use tempfile::TempDir;
use tokio::sync::{OnceCell, SetError};

#[cfg(not(test))]
static DB: Lazy<Surreal<Db>> = Lazy::new(|| {
    let db = Surreal::init();
    tokio::spawn(async {
        setup().await.unwrap();
    });
    db
});
#[cfg(test)]
static DB: Lazy<Surreal<Db>> = Lazy::new(|| {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    })
});

static DB_DIR: OnceCell<PathBuf> = OnceCell::const_new();

static TEMP_DB_DIR: Lazy<TempDir> =
    Lazy::new(|| tempfile::tempdir().expect("Failed to create temporary directory"));

pub async fn db() -> surrealdb::Result<&'static Surreal<Db>> {
    DB.wait_for(surrealdb::opt::WaitFor::Connection).await;
    DB.wait_for(surrealdb::opt::WaitFor::Database).await;
    Ok(DB.deref())
}

async fn setup() -> surrealdb::Result<()> {
    DB.connect( DB_DIR
        .get().cloned()
        .unwrap_or_else(|| {
            log::warn!("DB_DIR not set, defaulting to a temporary directory `{}`, this is likely a bug because `init_database` should be called before `db`", TEMP_DB_DIR.path().display());
            TEMP_DB_DIR.path()
            .to_path_buf()
        })).await?;
    DB.use_ns("mecomp").use_db("music").await?;

    register_tables!(
        DB.deref(),
        schemas::album::Album,
        schemas::artist::Artist,
        schemas::song::Song,
        schemas::collection::Collection,
        schemas::playlist::Playlist
    )?;

    Ok(())
}

pub async fn init_database(path: PathBuf) -> Result<(), SetError<PathBuf>> {
    DB_DIR.set(path)?;
    info!("Primed database path");
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
