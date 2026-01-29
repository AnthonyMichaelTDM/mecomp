//! Handles the configuration of the daemon.
//!
//! this module is responsible for parsing the Config.toml file, parsing cli arguments, and
//! setting up the logger.

use config::{Config, ConfigError, Environment, File};
use one_or_many::OneOrMany;
use serde::Deserialize;

use std::{num::NonZeroUsize, path::PathBuf, str::FromStr};

use mecomp_storage::util::MetadataConflictResolution;

pub static DEFAULT_CONFIG: &str = include_str!("../Mecomp.toml");

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
pub struct Settings {
    /// General Daemon Settings
    #[serde(default)]
    pub daemon: DaemonSettings,
    /// Settings for song analysis
    #[serde(default)]
    pub analysis: AnalysisSettings,
    /// Parameters for the reclustering algorithm.
    #[serde(default)]
    pub reclustering: ReclusterSettings,
    /// Settings for the TUI
    #[serde(default)]
    pub tui: TuiSettings,
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
    #[inline]
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

    /// Get the (default) path to the config file.
    /// If the config file does not exist at this path, it will be created with the default config.
    ///
    /// See [`crate::get_config_dir`] for more information about where this default path is located.
    ///
    /// # Errors
    ///
    /// This function will return an error if the system config directory (e.g., `~/.config` on linux) could not be found, or if the config file was missing and could not be created.
    #[inline]
    pub fn get_config_path() -> Result<PathBuf, std::io::Error> {
        match crate::get_config_dir() {
            Ok(config_dir) => {
                // if the config directory does not exist, create it
                if !config_dir.exists() {
                    std::fs::create_dir_all(&config_dir)?;
                }
                let config_file = config_dir.join("Mecomp.toml");

                if !config_file.exists() {
                    std::fs::write(&config_file, DEFAULT_CONFIG)?;
                }

                Ok(config_file)
            }
            Err(e) => {
                eprintln!("Error: {e}");
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Unable to find the config directory for mecomp.",
                ))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
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
    /// ...
    /// ```
    #[serde(default, deserialize_with = "de_artist_separator")]
    pub artist_separator: OneOrMany<String>,
    /// Exceptions for artist name separation, for example:
    /// "Foo & Bar; Baz" would be split into \["Foo", "Bar", "Baz"\] if the separators are set to "&" and "; ".
    ///
    /// However, if the following exception is set:
    /// ```toml
    /// [daemon]
    /// protected_artist_names = ["Foo & Bar"]
    /// ```
    /// Then the artist "Foo & Bar; Baz" would be split into \["Foo & Bar", "Baz"\].
    ///
    /// Note that the exception applies to the entire "name", so:
    /// ```toml
    /// [daemon]
    /// protected_artist_names = ["Foo & Bar"]
    /// ```
    /// would split "Foo & Bar" into \["Foo & Bar"\],
    /// but "Foo & Bar Baz" would still be split into \["Foo", "Bar Baz"\].
    #[serde(default)]
    pub protected_artist_names: OneOrMany<String>,
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
    #[serde(deserialize_with = "de_log_level")]
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

fn de_log_level<'de, D>(deserializer: D) -> Result<log::LevelFilter, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(log::LevelFilter::from_str(&s).unwrap_or_else(|_| default_log_level()))
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
    #[inline]
    fn default() -> Self {
        Self {
            rpc_port: default_port(),
            library_paths: default_library_paths(),
            artist_separator: OneOrMany::None,
            protected_artist_names: OneOrMany::None,
            genre_separator: None,
            conflict_resolution: MetadataConflictResolution::Overwrite,
            log_level: default_log_level(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisKind {
    #[default]
    Features,
    Embedding,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AnalysisSettings {
    /// The kind of analysis to perform, either "features" or "embedding".
    /// "features" will compute traditional audio features (tempo, key, etc.)
    /// "embedding" will compute neural audio embedding using a pre-trained model.
    /// Default is "features".
    ///
    /// Note that regardless of this setting, both features and embedding will be computed during analysis.
    /// This only determines the kind used for clustering, radio, and other such tasks
    #[serde(default)]
    pub kind: AnalysisKind,
    /// The number of threads to use for analysis.
    /// Default is the number of logical CPUs on the system.
    ///
    /// Note that:
    /// - increasing this number may increase memory usage significantly during analysis.
    /// - setting this number to more than the number of logical CPUs will have no effect (saturates at number of logical CPUs).
    /// - leave this unset to use the default.
    #[serde(default)]
    pub num_threads: Option<NonZeroUsize>,
    /// You can optionally override the model used for generating audio embeddings.
    /// Requirements:
    /// - The model must be in the ONNX format with opset version 16 or higher.
    /// - The model should expect mono audio samples at a sample rate of 22,050 Hz.
    /// - The input tensor must be name "audio" and have shape [B, N] where N a dynamic length corresponding to the number of audio samples in the song, and B is the batch size.
    /// - The output tensor must be name "embedding" and have shape [B, 32] corresponding to a 32-dimensional embedding vector. B is the batch size.
    ///
    /// If unset, or a non-existent/invalid path, the built-in model (which is bundled into the daemon binary) will be used.
    #[serde(default)]
    pub model_path: Option<PathBuf>,
}

impl Default for AnalysisSettings {
    #[inline]
    fn default() -> Self {
        Self {
            kind: AnalysisKind::default(),
            num_threads: None,
            model_path: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClusterAlgorithm {
    KMeans,
    #[default]
    GMM,
}

#[cfg(feature = "analysis")]
impl From<ClusterAlgorithm> for mecomp_analysis::clustering::ClusteringMethod {
    #[inline]
    fn from(algo: ClusterAlgorithm) -> Self {
        match algo {
            ClusterAlgorithm::KMeans => Self::KMeans,
            ClusterAlgorithm::GMM => Self::GaussianMixtureModel,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectionMethod {
    None,
    #[default]
    TSne,
    Pca,
}

#[cfg(feature = "analysis")]
impl From<ProjectionMethod> for mecomp_analysis::clustering::ProjectionMethod {
    #[inline]
    fn from(proj: ProjectionMethod) -> Self {
        match proj {
            ProjectionMethod::None => Self::None,
            ProjectionMethod::TSne => Self::TSne,
            ProjectionMethod::Pca => Self::Pca,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct ReclusterSettings {
    /// The number of reference datasets to use for the gap statistic.
    /// (which is used to determine the optimal number of clusters)
    /// 50 will give a decent estimate but for the best results use more,
    /// 500 will give a very good estimate but be very slow.
    /// We default to 250 in release mode.
    #[serde(default = "default_gap_statistic_reference_datasets")]
    pub gap_statistic_reference_datasets: u32,
    /// The maximum number of clusters to create.
    /// This is the upper bound on the number of clusters that can be created.
    /// Increase if you're getting a "could not find optimal k" error.
    /// Default is 24.
    #[serde(default = "default_max_clusters")]
    pub max_clusters: usize,
    /// The clustering algorithm to use.
    /// Either "kmeans" or "gmm".
    #[serde(default)]
    pub algorithm: ClusterAlgorithm,
    /// The projection method to preprocess the data with before clustering.
    /// Either "tsne", "pca", or "none".
    /// PCA works better with KMeans than it does with GMM
    /// Default is "tsne".
    #[serde(default)]
    pub projection_method: ProjectionMethod,
}

const fn default_gap_statistic_reference_datasets() -> u32 {
    50
}

const fn default_max_clusters() -> usize {
    #[cfg(debug_assertions)]
    return 16;
    #[cfg(not(debug_assertions))]
    return 24;
}

impl Default for ReclusterSettings {
    #[inline]
    fn default() -> Self {
        Self {
            gap_statistic_reference_datasets: default_gap_statistic_reference_datasets(),
            max_clusters: default_max_clusters(),
            algorithm: ClusterAlgorithm::default(),
            projection_method: ProjectionMethod::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TuiSettings {
    /// How many songs should be queried for when starting a radio.
    /// Default is 20.
    #[serde(default = "default_radio_count")]
    pub radio_count: u32,
    /// The color scheme to use for the TUI.
    /// Each color is either:
    /// - a hex string in the format `#RRGGBB`.
    ///   example: `#FFFFFF` for white.
    /// - a material design color name in format "<COLOR>_<SHADE>".
    ///   so "pink", `red-900`,  `light-blue_500`, `red900`, etc. are all invalid.
    ///   but `PINK_900`, `RED_900`, `LIGHT_BLUE_500` are valid.
    ///   - Exceptions are `WHITE` and `BLACK`, which are always valid.
    #[serde(default)]
    pub colors: TuiColorScheme,
}

const fn default_radio_count() -> u32 {
    20
}

impl Default for TuiSettings {
    #[inline]
    fn default() -> Self {
        Self {
            radio_count: default_radio_count(),
            colors: TuiColorScheme::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Default)]
pub struct TuiColorScheme {
    /// app border colors
    pub app_border: Option<String>,
    pub app_border_text: Option<String>,
    /// border colors
    pub border_unfocused: Option<String>,
    pub border_focused: Option<String>,
    /// popup border color
    pub popup_border: Option<String>,
    /// text colors
    pub text_normal: Option<String>,
    pub text_highlight: Option<String>,
    pub text_highlight_alt: Option<String>,
    /// gauge colors, such as song progress bar
    pub gauge_filled: Option<String>,
    pub gauge_unfilled: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    #[allow(dead_code)]
    #[serde(transparent)]
    struct ArtistSeparatorTest {
        #[serde(deserialize_with = "de_artist_separator")]
        artist_separator: OneOrMany<String>,
    }

    #[rstest]
    #[case(Vec::<String>::new())]
    #[case("")]
    fn test_de_artist_separator_empty<'de, D>(#[case] input: D)
    where
        D: serde::de::IntoDeserializer<'de>,
    {
        let deserializer = input.into_deserializer();
        let result: Result<OneOrMany<String>, _> = de_artist_separator(deserializer);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[rstest]
    #[case(vec![" & "], String::from(" & ").into())]
    #[case(" & ", String::from(" & ").into())]
    #[case(vec![" & ", "; "], vec![String::from(" & "), String::from("; ")].into())]
    #[case(vec!["", " & ", "", "; "], vec![String::from(" & "), String::from("; ")].into())]
    fn test_de_artist_separator<'de, D>(#[case] input: D, #[case] expected: OneOrMany<String>)
    where
        D: serde::de::IntoDeserializer<'de>,
    {
        let deserializer = input.into_deserializer();
        let result: Result<OneOrMany<String>, _> = de_artist_separator(deserializer);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_init_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"            
[daemon]
rpc_port = 6600
library_paths = ["/Music"]
artist_separator = ["; "]
genre_separator = ", "
conflict_resolution = "overwrite"
log_level = "debug"

[reclustering]
gap_statistic_reference_datasets = 50
max_clusters = 24
algorithm = "gmm"

[tui]
radio_count = 21
[tui.colors]
app_border = "PINK_900"
app_border_text = "PINK_300"
border_unfocused = "RED_900"
border_focused = "RED_200"
popup_border = "LIGHT_BLUE_500"
text_normal = "WHITE"
text_highlight = "RED_600"
text_highlight_alt = "RED_200"
gauge_filled = "WHITE"
gauge_unfilled = "BLACK"
            "#,
        )
        .unwrap();

        let expected = Settings {
            daemon: DaemonSettings {
                rpc_port: 6600,
                library_paths: ["/Music".into()].into(),
                artist_separator: vec!["; ".into()].into(),
                protected_artist_names: OneOrMany::None,
                genre_separator: Some(", ".into()),
                conflict_resolution: MetadataConflictResolution::Overwrite,
                log_level: log::LevelFilter::Debug,
            },
            analysis: AnalysisSettings::default(),
            reclustering: ReclusterSettings {
                gap_statistic_reference_datasets: 50,
                max_clusters: 24,
                algorithm: ClusterAlgorithm::GMM,
                projection_method: ProjectionMethod::None,
            },
            tui: TuiSettings {
                radio_count: 21,
                colors: TuiColorScheme {
                    app_border: Some("PINK_900".into()),
                    app_border_text: Some("PINK_300".into()),
                    border_unfocused: Some("RED_900".into()),
                    border_focused: Some("RED_200".into()),
                    popup_border: Some("LIGHT_BLUE_500".into()),
                    text_normal: Some("WHITE".into()),
                    text_highlight: Some("RED_600".into()),
                    text_highlight_alt: Some("RED_200".into()),
                    gauge_filled: Some("WHITE".into()),
                    gauge_unfilled: Some("BLACK".into()),
                },
            },
        };

        let settings = Settings::init(config_path, None, None).unwrap();

        assert_eq!(settings, expected);
    }

    #[test]
    fn test_tui_colors_unset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"            
[daemon]
rpc_port = 6600
library_paths = ["/Music"]
artist_separator = ["; "]
protected_artist_names = ["Foo & Bar"]
genre_separator = ", "
conflict_resolution = "overwrite"
log_level = "debug"

[reclustering]
gap_statistic_reference_datasets = 50
max_clusters = 24
algorithm = "gmm"

[tui]
radio_count = 21
            "#,
        )
        .unwrap();

        let expected = Settings {
            daemon: DaemonSettings {
                rpc_port: 6600,
                library_paths: ["/Music".into()].into(),
                artist_separator: vec!["; ".into()].into(),
                protected_artist_names: "Foo & Bar".to_string().into(),
                genre_separator: Some(", ".into()),
                conflict_resolution: MetadataConflictResolution::Overwrite,
                log_level: log::LevelFilter::Debug,
            },
            analysis: AnalysisSettings::default(),
            reclustering: ReclusterSettings {
                gap_statistic_reference_datasets: 50,
                max_clusters: 24,
                algorithm: ClusterAlgorithm::GMM,
                projection_method: ProjectionMethod::None,
            },
            tui: TuiSettings {
                radio_count: 21,
                colors: TuiColorScheme::default(),
            },
        };

        let settings = Settings::init(config_path, None, None).unwrap();

        assert_eq!(settings, expected);
    }

    #[test]
    fn test_artist_names_to_not_split() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"            
[daemon]
rpc_port = 6600
library_paths = ["/Music"]
artist_separator = ["; "]
protected_artist_names = ["Foo & Bar"]
genre_separator = ", "
conflict_resolution = "overwrite"
log_level = "debug"

[reclustering]
gap_statistic_reference_datasets = 50
max_clusters = 24
algorithm = "gmm"

[tui]
radio_count = 21
            "#,
        )
        .unwrap();

        let expected = Settings {
            daemon: DaemonSettings {
                rpc_port: 6600,
                library_paths: ["/Music".into()].into(),
                artist_separator: vec!["; ".into()].into(),
                protected_artist_names: "Foo & Bar".to_string().into(),
                genre_separator: Some(", ".into()),
                conflict_resolution: MetadataConflictResolution::Overwrite,
                log_level: log::LevelFilter::Debug,
            },
            analysis: AnalysisSettings::default(),
            reclustering: ReclusterSettings {
                gap_statistic_reference_datasets: 50,
                max_clusters: 24,
                algorithm: ClusterAlgorithm::GMM,
                projection_method: ProjectionMethod::None,
            },
            tui: TuiSettings {
                radio_count: 21,
                colors: TuiColorScheme::default(),
            },
        };

        let settings = Settings::init(config_path, None, None).unwrap();

        assert_eq!(settings, expected);
    }

    #[test]
    fn test_default_config_works() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(&config_path, DEFAULT_CONFIG).unwrap();

        let settings = Settings::init(config_path, None, None);

        assert!(settings.is_ok(), "Error: {:?}", settings.err());
    }
}
