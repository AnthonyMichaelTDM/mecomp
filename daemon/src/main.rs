//! This is the daemon binary that runs the Mecomp daemon.
//! there are no tests or anything else in this file because the only thing it does is set up and start the daemon
//! with functions from the `mecomp_daemon` library crate (which is tested).

use std::path::PathBuf;

use mecomp_core::{get_config_dir, get_data_dir};
use mecomp_daemon::{config::DaemonSettings, start_daemon};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags = Flags::try_parse()?;

    let config_file = match get_config_dir() {
        Ok(config_dir) => config_dir.join("Mecomp.toml"),
        Err(e) => {
            eprintln!("Error: {e}");
            // TODO: once this thing is released, maybe make this an actual error?
            PathBuf::from("Mecomp.toml")
        }
    };

    let db_dir = match get_data_dir() {
        Ok(data_dir) => data_dir.join("db"),
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Using a temporary directory for the database");
            std::env::temp_dir().join("mecomp")
        }
    };

    let settings = DaemonSettings::init(flags.port, flags.config.unwrap_or(config_file))?;

    start_daemon(log::LevelFilter::Debug, settings, db_dir).await
}
