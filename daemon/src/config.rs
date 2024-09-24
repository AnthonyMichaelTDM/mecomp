//! Handles the configuration of the daemon.
//!
//! this module is responsible for parsing the Config.toml file, parsing cli arguments, and
//! setting up the logger.

use config::{Config, ConfigError, Environment, File};
use one_or_many::OneOrMany;
use serde::Deserialize;

use std::path::PathBuf;

use mecomp_storage::util::MetadataConflictResolution;

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Settings {
    /// General Daemon Settings
    #[serde(default)]
    pub daemon: DaemonSettings,
    /// Parameters for the reclustering algorithm.
    #[serde(default)]
    pub reclustering: ReclusterSettings,
}

impl Settings {
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

        for path in &mut settings.daemon.library_paths {
            *path = shellexpand::tilde(&path.to_string_lossy())
                .into_owned()
                .into();
        }

        if let Some(port) = port {
            settings.daemon.rpc_port = port;
        }

        if let Some(log_level) = log_level {
            settings.daemon.log_level = log_level;
        }

        Ok(settings)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DaemonSettings {
    /// The port to listen on for RPC requests.
    /// Default is 6600.
    #[serde(default = "default_port")]
    pub rpc_port: u16,
    /// The root paths of the music library.
    #[serde(default = "default_library_paths")]
    pub library_paths: Box<[PathBuf]>,
    /// Separators for artist names in song metadata.
    /// For example, "Foo, Bar, Baz" would be split into \["Foo", "Bar", "Baz"\]. if the separator is ", ".
    /// If the separator is not found, the entire string is considered as a single artist.
    /// If unset, will not split artists.
    ///
    /// Users can provide one or many separators, and must provide them as either a single string or an array of strings.
    ///
    /// ```toml
    /// [daemon]
    /// artist_separator = " & "
    /// artist_separator = [" & ", "; "]
    ///
    ///
    /// ```
    #[serde(default, deserialize_with = "de_artist_separator")]
    pub artist_separator: OneOrMany<String>,
    #[serde(default)]
    pub genre_separator: Option<String>,
    /// how conflicting metadata should be resolved
    /// "overwrite" - overwrite the metadata with new metadata
    /// "skip" - skip the file (keep old metadata)
    #[serde(default)]
    pub conflict_resolution: MetadataConflictResolution,
    /// What level of logging to use.
    /// Default is "info".
    #[serde(default = "default_log_level")]
    pub log_level: log::LevelFilter,
}

fn de_artist_separator<'de, D>(deserializer: D) -> Result<OneOrMany<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = OneOrMany::<String>::deserialize(deserializer)?
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<OneOrMany<String>>();
    if v.is_empty() {
        Ok(OneOrMany::None)
    } else {
        Ok(v)
    }
}

const fn default_port() -> u16 {
    6600
}

fn default_library_paths() -> Box<[PathBuf]> {
    vec![shellexpand::tilde("~/Music/").into_owned().into()].into_boxed_slice()
}

const fn default_log_level() -> log::LevelFilter {
    log::LevelFilter::Info
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            rpc_port: default_port(),
            library_paths: default_library_paths(),
            artist_separator: OneOrMany::None,
            genre_separator: None,
            conflict_resolution: MetadataConflictResolution::Overwrite,
            log_level: default_log_level(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ReclusterSettings {
    /// The number of reference datasets to use for the gap statistic.
    /// (which is used to determine the optimal number of clusters)
    /// 50 will give a decent estimate but for the best results use more,
    /// 500 will give a very good estimate but be very slow.
    /// We default to 250 in release mode.
    #[serde(default = "default_gap_statistic_reference_datasets")]
    pub gap_statistic_reference_datasets: usize,
    /// The maximum number of clusters to create.
    /// This is the upper bound on the number of clusters that can be created.
    /// Increase if you're getting a "could not find optimal k" error.
    /// Default is 24.
    #[serde(default = "default_max_clusters")]
    pub max_clusters: usize,
    /// The maximum number of iterations to run the k-means algorithm.
    /// Shouldn't be less than 30, but can be increased.
    /// A good value is the number of songs in your library, divided by 10.
    /// Default is 120.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
}

const fn default_gap_statistic_reference_datasets() -> usize {
    #[cfg(debug_assertions)]
    return 50;
    #[cfg(not(debug_assertions))]
    return 250;
}

const fn default_max_clusters() -> usize {
    #[cfg(debug_assertions)]
    return 16;
    #[cfg(not(debug_assertions))]
    return 24;
}

const fn default_max_iterations() -> usize {
    #[cfg(debug_assertions)]
    return 30;
    #[cfg(not(debug_assertions))]
    return 120;
}

impl Default for ReclusterSettings {
    fn default() -> Self {
        Self {
            gap_statistic_reference_datasets: default_gap_statistic_reference_datasets(),
            max_clusters: default_max_clusters(),
            max_iterations: default_max_iterations(),
        }
    }
}
