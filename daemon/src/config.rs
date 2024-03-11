//! Handles the configuration of the daemon.
//!
//! this module is responsible for parsing the Config.toml file, parsing cli arguments, and
//! setting up the logger.

use clap::Parser;
use config::{Config, ConfigError, Environment, File};
use lazy_static::lazy_static;
use serde::Deserialize;

use std::{path::PathBuf, sync::Arc};

use mecomp_storage::util::MetadataConflictResolution;

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

#[derive(Clone, Debug, Deserialize)]
pub struct DaemonSettings {
    /// The port to listen on for RPC requests.
    pub rpc_port: u16,
    /// The path to the database.
    pub db_path: PathBuf,
    /// The root paths of the music library.
    pub library_paths: Vec<PathBuf>,
    /// Sepators for artist names in song metadata.
    /// For example, "Foo, Bar, Baz" would be split into ["Foo", "Bar", "Baz"]. if the separator is ", ".
    /// If the separator is not found, the entire string is considered as a single artist.
    /// If unset, will not split artists.
    pub artist_separator: Option<&'static str>,
    pub genre_separator: Option<&'static str>,
    /// how conflicting metadata should be resolved
    /// "merge" - merge the metadata
    /// "overwrite" - overwrite the metadata with new metadata
    /// "skip" - skip the file (keep old metadata)
    pub conflict_resolution: MetadataConflictResolution,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            rpc_port: 6600,
            db_path: PathBuf::from("/tmp/mecomp_db"),
            library_paths: vec![PathBuf::from("~/Music")],
            artist_separator: None,
            genre_separator: None,
            conflict_resolution: MetadataConflictResolution::Merge,
        }
    }
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

lazy_static! {
    pub static ref SETTINGS: Arc<DaemonSettings> =
        Arc::new(DaemonSettings::init().unwrap_or_default());
}
