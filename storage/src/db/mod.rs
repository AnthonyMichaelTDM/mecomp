pub mod crud;
pub mod schemas;

use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use once_cell::sync::Lazy;
use surrealdb::engine::local::{Db, SpeeDb};
use surrealdb::Surreal;
use tempfile::TempDir;
use tokio::sync::{Mutex, OnceCell, SetError};

/// what I need is a type that allows for:
/// 1. lazy initialization
/// 2. static lifetime
/// 3. need to be able to send CLONEs of the Surreal<Db> instance to different threads
/// 4. need to be able to safely sync the wrapper instance across threads
/// 5. need to be able to modify the underlying Surreal<Db> when the connection is dropped WHILE MAINTAINING THREAD SAFETY AND AVOIDING DEADLOCKS
///
/// hopefully, this should do the trick
///
/// TODO: find a more performant way to handle our locks, maybe an atomic type or something?
struct DbWrapper {
    /// the actual database instance, mutex allows thread-safe mutability.
    db: Mutex<Surreal<Db>>,
    /// a lock to ensure that we don't take a lock on `db` while we want to write to it (maybe not necessary, but better safe than sorry)
    read_lock: Mutex<()>,
}

impl DbWrapper {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(Surreal::<Db>::init()),
            read_lock: Mutex::new(()),
        }
    }

    async fn connect(&self, path_fn: impl FnOnce() -> PathBuf) -> surrealdb::Result<()> {
        match self.db.try_lock() {
            Ok(mut guard) => {
                // Ideally, we'd be able to directly end the current connection without dropping the Surreal<Db> instance
                // however, I'm not sure if that's possible, which is why we need to rely on these locks and interior mutability
                // to ensure we don't accidently cause a race condition, deadlock, or other such unpleasantness
                guard.clone_from(&Surreal::init());
                guard.connect::<SpeeDb>(path_fn()).await?;
                guard.use_ns("mecomp").use_db("music").await?;
                info!("Connected to music database");
            }
            Err(_) => {
                log::debug!("Connection already in progress, aborting...");
            }
        }
        Ok(())
    }

    /// this being the only public function that can acquire locks helps assure that we don't accidentally cause a deadlock
    pub async fn get_or_reconnect(
        &self,
        path_fn: impl FnOnce() -> PathBuf,
    ) -> surrealdb::Result<Surreal<Db>> {
        let _read_guard = self.read_lock.lock().await;

        if self.db.lock().await.health().await.is_err() {
            log::debug!("Attempting to reconnect to database...");
            self.connect(path_fn).await?;
        }
        let guard = self.db.lock().await;
        guard.wait_for(surrealdb::opt::WaitFor::Connection).await;
        guard.wait_for(surrealdb::opt::WaitFor::Database).await;
        Ok((*guard).clone())
    }
}

// since we've taken steps to ensure thread safety, we can tell the compiler that this is safe

static DB: Lazy<Arc<DbWrapper>> = Lazy::new(|| Arc::new(DbWrapper::new()));
static DB_DIR: Lazy<OnceCell<PathBuf>> = Lazy::new(OnceCell::new);

static TEMP_DB_DIR: Lazy<TempDir> =
    Lazy::new(|| tempfile::tempdir().expect("Failed to create temporary directory"));

pub async fn db() -> surrealdb::Result<Surreal<Db>> {
    match DB.get_or_reconnect(|| DB_DIR
        .get().cloned()
        .unwrap_or_else(|| {
            log::warn!("DB_DIR not set, defaulting to a temporary directory, this is likely a bug because `init_database` should be called before `db`");
            TEMP_DB_DIR.path()
            .to_path_buf()
        })).await {
            Ok(db) => Ok(db),
            Err(err) => {
                log::error!("Failed to get database connection: {:?}", err);
                Err(err)
            },
        }
}

pub async fn init_database(path: PathBuf) -> Result<(), SetError<PathBuf>> {
    DB_DIR.set(path)?;
    info!("Primed database path");
    Ok(())
}
