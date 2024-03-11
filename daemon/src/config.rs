//! Handles the configuration of the daemon.
//!
//! this module is responsible for parsing the Config.toml file, parsing cli arguments, and
//! setting up the logger.

use clap::Parser;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

/// Options configurable via the CLI.
#[derive(Parser)]
pub struct Flags {
    /// Sets the port number to listen on.
    #[clap(long)]
    port: Option<u16>,
    /// config file path
    #[clap(long, default_value = "Mecomp.toml")]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct DaemonSettings {
    /// The port to listen on for RPC requests.
    pub rpc_port: u16,
    /// The path to the database.
    pub db_path: PathBuf,
    /// The path to the music library.
    pub library_paths: Vec<PathBuf>,
    /// Sepators for artist names in song metadata.
    /// For example, "Foo, Bar, Baz" would be split into ["Foo", "Bar", "Baz"]. if the separator is ", ".
    /// If the separator is not found, the entire string is considered as a single artist.
    /// If unset, will not split artists.
    pub artist_separator: Option<&'static str>,
}

impl DaemonSettings {
    pub fn init() -> Result<Self, ConfigError> {
        let flags = Flags::parse();

        let s = Config::builder()
            .add_source(File::from(flags.config))
            .add_source(Environment::with_prefix("MECOMP"))
            .build()?;

        let mut settings: Self = s.try_deserialize()?;

        if let Some(port) = flags.port {
            settings.rpc_port = port;
        }

        Ok(settings)
    }
}
