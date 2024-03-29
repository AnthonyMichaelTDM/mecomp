pub mod crud;
pub mod schemas;

use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use once_cell::sync::Lazy;
use surrealdb::engine::local::{Db, SpeeDb};
use surrealdb::Surreal;
use tempfile::TempDir;
use tokio::sync::{OnceCell, SetError};

static DB: Lazy<OnceCell<Arc<Surreal<Db>>>> = Lazy::new(OnceCell::new);
static DB_DIR: Lazy<OnceCell<PathBuf>> = Lazy::new(OnceCell::new);

static TEMP_DB_DIR: Lazy<TempDir> =
    Lazy::new(|| tempfile::tempdir().expect("Failed to create temporary directory"));

pub async fn db() -> Arc<Surreal<Db>> {
    DB.get_or_init(|| async {
        let db = Surreal::new::<SpeeDb>(
            DB_DIR
                .get().cloned()
                .unwrap_or_else(|| {
                    log::warn!("DB_DIR not set, defaulting to a temporary directory, this is likely a bug because `init_database` should be called before `db`");
                    TEMP_DB_DIR.path()
                    .to_path_buf()
                }),
        ).with_capacity(0).await.unwrap();
        db.use_ns("mecomp").await.unwrap();
        db.use_db("music").await.unwrap();
        info!("Connected to music database");
        Arc::new(db)
    })
    .await.clone()
}

pub async fn init_database(path: PathBuf) -> Result<(), SetError<PathBuf>> {
    DB_DIR.set(path)?;
    info!("Primed database path");
    Ok(())
}
