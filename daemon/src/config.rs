//! Handles the configuration of the daemon.
//!
//! this module is responsible for parsing the Config.toml file, parsing cli arguments, and
//! setting up the logger.

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use std::path::PathBuf;

use mecomp_storage::util::MetadataConflictResolution;

const DEFAULT_PORT: u16 = 6600;

const fn default_port() -> u16 {
    DEFAULT_PORT
}

const fn default_log_level() -> log::LevelFilter {
    log::LevelFilter::Info
}

#[derive(Clone, Debug, Deserialize)]
pub struct DaemonSettings {
    /// The port to listen on for RPC requests.
    /// Default is 6600.
    #[serde(default = "default_port")]
    pub rpc_port: u16,
    /// The root paths of the music library.
    pub library_paths: Box<[PathBuf]>,
    /// Sepators for artist names in song metadata.
    /// For example, "Foo, Bar, Baz" would be split into \["Foo", "Bar", "Baz"\]. if the separator is ", ".
    /// If the separator is not found, the entire string is considered as a single artist.
    /// If unset, will not split artists.
    pub artist_separator: Option<String>,
    pub genre_separator: Option<String>,
    /// how conflicting metadata should be resolved
    /// "merge" - merge the metadata
    /// "overwrite" - overwrite the metadata with new metadata
    /// "skip" - skip the file (keep old metadata)
    pub conflict_resolution: MetadataConflictResolution,
    /// What level of logging to use.
    /// Default is "info".
    #[serde(default = "default_log_level")]
    pub log_level: log::LevelFilter,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            rpc_port: 6600,
            library_paths: vec![shellexpand::tilde("~/Music/").into_owned().into()]
                .into_boxed_slice(),
            artist_separator: None,
            genre_separator: None,
            conflict_resolution: MetadataConflictResolution::Overwrite,
            log_level: log::LevelFilter::Info,
        }
    }
}

impl DaemonSettings {
    /// Load settings from the config file, environment variables, and CLI arguments.
    ///
    /// The config file is located at the path specified by the `--config` flag.
    ///
    /// The environment variables are prefixed with `MECOMP_`.
    ///
    /// # Arguments
    ///
    /// * `flags` - The parsed CLI arguments.
    ///
    /// # Errors
    ///
    /// This function will return an error if the config file is not found or if the config file is
    /// invalid.
    pub fn init(
        config: PathBuf,
        port: Option<u16>,
        log_level: Option<log::LevelFilter>,
    ) -> Result<Self, ConfigError> {
        let s = Config::builder()
            .add_source(File::from(config))
            .add_source(Environment::with_prefix("MECOMP"))
            .build()?;

        let mut settings: Self = s.try_deserialize()?;

        for path in settings.library_paths.iter_mut() {
            *path = shellexpand::tilde(&path.to_string_lossy())
                .into_owned()
                .into();
        }

        if let Some(port) = port {
            settings.rpc_port = port;
        }

        if let Some(log_level) = log_level {
            settings.log_level = log_level;
        }

        Ok(settings)
    }
}
