//! This is the daemon binary that runs the Mecomp daemon.
//! there are no tests or anything else in this file because the only thing it does is set up and start the daemon
//! with functions from the `mecomp_daemon` library crate (which is tested).

use std::path::PathBuf;

use mecomp_core::{get_config_dir, get_data_dir};
use mecomp_daemon::{
    config::{Settings, DEFAULT_CONFIG},
    start_daemon,
};

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

    let config_file = match get_config_dir() {
        Ok(config_dir) => {
            // if the config directory does not exist, create it
            if !config_dir.exists() {
                std::fs::create_dir_all(&config_dir)?;
            }
            config_dir.join("Mecomp.toml")
        }
        Err(e) => {
            eprintln!("Error: {e}");
            anyhow::bail!("Could not find the config directory")
        }
    };

    // write the default config file if one does not exist
    if !config_file.exists() {
        std::fs::write(&config_file, DEFAULT_CONFIG)?;
    }

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
