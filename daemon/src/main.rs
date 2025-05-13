//! This is the daemon binary that runs the Mecomp daemon.
//! there are no tests or anything else in this file because the only thing it does is set up and start the daemon
//! with functions from the `mecomp_daemon` library crate (which is tested).

use std::path::PathBuf;

use clap::{
    CommandFactory, Parser,
    builder::{PossibleValuesParser, TypedValueParser},
};
use log::LevelFilter;

use mecomp_core::{config::Settings, get_data_dir};
use mecomp_daemon::start_daemon;

#[cfg(not(feature = "cli"))]
compile_error!("The cli feature is required to build the daemon binary");

/// Options configurable via the CLI.
#[derive(Parser)]
#[command(name = "mecomp-daemon", version = env!("CARGO_PKG_VERSION"), about)]
struct Flags {
    /// Set the TCP port that the daemon will listen on
    #[clap(
        long,
        value_hint = clap::ValueHint::Other,
    )]
    port: Option<u16>,
    /// config file path
    #[clap(
        long,
        short,
        help = "Use this config file instead of the one in the default location",
        value_hint = clap::ValueHint::FilePath,
    )]
    config: Option<PathBuf>,
    /// Set the log level
    #[clap(
        long,
        short,
        value_parser = PossibleValuesParser::new([ "off", "trace", "debug", "info", "warn", "error"])
            .map(|s| s.parse::<LevelFilter>().unwrap())
    )]
    log_level: Option<LevelFilter>,
    /// Optionally disable the queue persistence mechanism
    #[clap(long, help = "Disable the queue persistence mechanism", action = clap::ArgAction::SetTrue)]
    no_persistence: bool,
}

fn main() -> anyhow::Result<()> {
    clap_complete::CompleteEnv::with_factory(Flags::command).complete();

    let flags = Flags::try_parse()?;

    let config_file: PathBuf = match &flags.config {
        Some(config_file) if config_file.exists() => config_file.clone(),
        Some(_) => anyhow::bail!("Config file does not exist at user specified path"),
        None => Settings::get_config_path()?,
    };

    assert!(config_file.exists(), "Config file does not exist");

    let (db_dir, log_file, state_file) = match get_data_dir() {
        Ok(data_dir) => {
            // if the data directory does not exist, create it
            if !data_dir.exists() {
                std::fs::create_dir_all(&data_dir)?;
            }
            (
                data_dir.join("db"),
                Some(data_dir.join("mecomp.log")),
                if flags.no_persistence {
                    None
                } else {
                    Some(data_dir.join("mecomp.state.json"))
                },
            )
        }
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Using a temporary directory for the database");
            let data_dir = std::env::temp_dir();
            (data_dir.join("mecomp_db"), None, None)
        }
    };

    let settings = Settings::init(config_file, flags.port, flags.log_level)?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(mecomp_daemon::MAX_CONCURRENT_REQUESTS)
        // SurrealDB recommends a 10MB stack size, but I think that's a bit much for our use case
        // (we're not processing millions of records)
        // .thread_stack_size(10 * 1024 * 1024) // 10MB
        .build()?
        .block_on(start_daemon(settings, db_dir, log_file, state_file))?;

    println!("exiting");
    Ok(())
}
