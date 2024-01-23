use log::info;
use once_cell::sync::Lazy;
use surrealdb::engine::local::{Db, SpeeDb};
use surrealdb::Surreal;

pub mod schemas;

static DB: Lazy<Surreal<Db>> = Lazy::new(|| Surreal::init());

fn db_path() -> String {
    // TODO: Make this configurable.
    std::env::var("MECOMP_DB_PATH").unwrap_or_else(|_| "/tmp/mecomp_db".to_string())
}

pub async fn init_database() -> surrealdb::Result<()> {
    DB.connect::<SpeeDb>(db_path()).await?;
    DB.use_ns("mecomp").await?;
    info!("Connected to music database");

    Ok(())
}
