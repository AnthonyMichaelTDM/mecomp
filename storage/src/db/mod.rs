pub mod crud;
pub mod errors;
pub mod schemas;

use std::path::PathBuf;

use log::info;
use once_cell::sync::Lazy;
use surrealdb::engine::local::{Db, SpeeDb};
use surrealdb::Surreal;

pub(crate) static DB: Lazy<Surreal<Db>> = Lazy::new(|| Surreal::init());

pub async fn init_database(path: PathBuf) -> surrealdb::Result<()> {
    DB.connect::<SpeeDb>(path).await?;
    DB.use_ns("mecomp").await?;
    info!("Connected to music database");

    Ok(())
}
