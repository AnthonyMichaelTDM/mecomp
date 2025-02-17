//! This is the daemon binary that runs the Mecomp daemon.
//! there are no tests or anything else in this file because the only thing it does is set up and start the daemon
//! with functions from the `mecomp_daemon` library crate (which is tested).

use std::path::PathBuf;

use mecomp_core::{config::Settings, get_data_dir};
use mecomp_daemon::start_daemon;

use clap::Parser;

#[cfg(not(feature = "cli"))]
compile_error!("The cli feature is required to build the daemon binary");

/// Options configurable via the CLI.
#[derive(Parser)]
struct Flags {
    /// Sets the port number to listen on.
    #[clap(long)]
    port: Option<u16>,
    /// config file path
    #[clap(long)]
    config: Option<PathBuf>,
    /// log level
    #[clap(long)]
    log_level: Option<log::LevelFilter>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags = Flags::try_parse()?;

    let config_file = match flags.config {
        Some(ref config_file) if config_file.exists() => config_file.clone(),
        Some(_) => anyhow::bail!("Config file does not exist at user specified path"),
        None => Settings::get_config_path()?,
    };

    assert!(config_file.exists(), "Config file does not exist");

    let (db_dir, log_file) = match get_data_dir() {
        Ok(data_dir) => {
            // if the data directory does not exist, create it
            if !data_dir.exists() {
                std::fs::create_dir_all(&data_dir)?;
            }
            (data_dir.join("db"), data_dir.join("mecomp.log"))
        }
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Using a temporary directory for the database");
            let data_dir = std::env::temp_dir();
            (data_dir.join("mecomp_db"), data_dir.join("mecomp.log"))
        }
    };

    let settings = Settings::init(
        flags.config.unwrap_or(config_file),
        flags.port,
        flags.log_level,
    )?;

    start_daemon(settings, db_dir, Some(log_file)).await
}
