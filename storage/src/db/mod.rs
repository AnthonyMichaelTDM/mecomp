pub mod crud;
pub mod schemas;

use std::{ops::Deref, path::PathBuf};

use log::info;
use once_cell::sync::Lazy;
use surrealdb::{engine::local::Db, Surreal};
use surrealqlx::register_tables;
use tempfile::TempDir;
use tokio::sync::{OnceCell, SetError};

static DB: Lazy<Surreal<Db>> = Lazy::new(|| {
    tokio::spawn(async {
        setup().await.unwrap();
    });
    Surreal::init()
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
