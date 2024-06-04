//! This is the daemon binary that runs the Mecomp daemon.
//! there are no tests or anything else in this file because the only thing it does is set up and start the daemon
//! with functions from the mecomp_daemon library crate (which is tested).

use std::path::PathBuf;

use mecomp_daemon::{config::DaemonSettings, start_daemon};

use clap::Parser;

#[cfg(not(feature = "clap"))]
compile_error!("The clap feature is required to build the daemon binary");

/// Options configurable via the CLI.
#[derive(Parser)]
struct Flags {
    /// Sets the port number to listen on.
    #[clap(long)]
    port: Option<u16>,
    /// config file path
    #[clap(long, default_value = "Mecomp.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags = Flags::try_parse()?;

    let settings = DaemonSettings::init(flags.port, flags.config)?;

    start_daemon(log::LevelFilter::Debug, settings).await
}
