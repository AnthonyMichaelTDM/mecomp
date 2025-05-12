#![deny(clippy::missing_inline_in_public_items)]

use errors::DirectoryError;

#[cfg(feature = "audio")]
pub mod audio;
pub mod config;
pub mod errors;
pub mod logger;
#[cfg(feature = "rpc")]
pub mod rpc;
pub mod state;
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;
#[cfg(feature = "rpc")]
pub mod udp;

#[cfg(test)]
extern crate rstest_reuse;

/// This macro returns the name of the enclosing function.
/// As the internal implementation is based on the [`std::any::type_name`], this macro derives
/// all the limitations of this function.
///
/// ## Examples
///
/// ```rust
/// mod bar {
///     pub fn sample_function() {
///         use mecomp_core::function_name;
///         assert!(function_name!().ends_with("bar::sample_function"));
///     }
/// }
///
/// bar::sample_function();
/// ```
///
/// [`std::any::type_name`]: https://doc.rust-lang.org/std/any/fn.type_name.html
///
/// # Note
///
/// This macro is copied from the `stdext` crate. <https://github.com/popzxc/stdext-rs>
#[macro_export]
macro_rules! function_name {
    () => {{
        // Okay, this is ugly, I get it. However, this is the best we can get on a stable rust.
        const fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // `3` is the length of the `::f`.
        &name[..name.len() - 3]
    }};
}

#[must_use]
#[inline]
pub fn format_duration(duration: &std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = duration.as_secs_f32() % 60.;

    format!("{hours:02}:{minutes:02}:{seconds:05.2}")
}

/// Get the data directory for the application.
///
/// Follows the XDG Base Directory Specification for linux, and the equivalents on other platforms.
/// See the [`directories`](https://docs.rs/directories/latest/directories/) crate for more information.
///
/// # Errors
///
/// This function will return an error if the data directory could not be found.
#[inline]
pub fn get_data_dir() -> Result<std::path::PathBuf, DirectoryError> {
    let directory = if let Ok(s) = std::env::var("MECOMP_DATA") {
        std::path::PathBuf::from(s)
    } else if let Some(proj_dirs) =
        directories::ProjectDirs::from("com", "anthonymichaeltdm", "mecomp")
    {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        return Err(DirectoryError::Data);
    };
    Ok(directory)
}

/// Get the config directory for the application.
///
/// Follows the XDG Base Directory Specification for linux, and the equivalents on other platforms.
/// See the [`directories`](https://docs.rs/directories/latest/directories/) crate for more information.
///
/// # Errors
///
/// This function will return an error if the config directory could not be found.
#[inline]
pub fn get_config_dir() -> Result<std::path::PathBuf, DirectoryError> {
    let directory = if let Ok(s) = std::env::var("MECOMP_CONFIG") {
        std::path::PathBuf::from(s)
    } else if let Some(proj_dirs) =
        directories::ProjectDirs::from("com", "anthonymichaeltdm", "mecomp")
    {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        return Err(DirectoryError::Config);
    };
    Ok(directory)
}

/// Check if a server is already running on localhost on the given port.
/// If a server is already running, return true, otherwise return false.
#[must_use]
#[inline]
pub fn is_server_running(port: u16) -> bool {
    std::net::TcpStream::connect(format!("localhost:{port}")).is_ok()
}

/// A `OnceLock` that returns a default value if it has not been set yet.
#[derive(Debug, Clone)]
pub struct OnceLockDefault<T> {
    value: std::sync::OnceLock<T>,
    default: T,
}

impl<T> OnceLockDefault<T> {
    /// Creates a new `OnceLockDefault` with the given default value.
    #[inline]
    pub const fn new(default: T) -> Self {
        Self {
            value: std::sync::OnceLock::new(),
            default,
        }
    }

    /// Initializes the contents of the cell to value.
    ///
    /// May block if another thread is currently attempting to initialize the cell.
    /// The cell is guaranteed to contain a value when set returns, though not necessarily the one provided.
    ///
    /// # Errors
    ///
    /// Returns `Ok(())` if the cell was uninitialized and `Err(value)` if the cell was already initialized.
    #[inline]
    pub fn set(&self, value: T) -> Result<(), T> {
        self.value.set(value)
    }

    /// Gets the reference to the underlying value, if set. Otherwise returns a reference to the default value.
    ///
    /// This method never blocks.
    #[inline]
    pub fn get(&self) -> &T {
        self.value.get().unwrap_or(&self.default)
    }

    /// Checks if the cell has been initialized.
    ///
    /// This method never blocks.
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.value.get().is_some()
    }
}

impl<T> std::ops::Deref for OnceLockDefault<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

#[cfg(test)]
mod test {
    use super::format_duration;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::time::Duration;

    #[rstest]
    #[case::zero(Duration::from_secs(0), "00:00:00.00")]
    #[case::sub_second(Duration::from_millis(100), "00:00:00.10")]
    #[case::sub_second(Duration::from_millis(101), "00:00:00.10")]
    #[case::one_second(Duration::from_secs(1), "00:00:01.00")]
    #[case::one_minute(Duration::from_secs(60), "00:01:00.00")]
    #[case::one_hour(Duration::from_secs(3600), "01:00:00.00")]
    #[case::one_hour_one_minute_one_second(Duration::from_secs(3661), "01:01:01.00")]
    #[case(Duration::from_secs(3600 + 120 + 1), "01:02:01.00")]
    fn test_format_duration(#[case] duration: Duration, #[case] expected: &str) {
        let actual = format_duration(&duration);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_name() {
        fn test_function() {
            let result = super::function_name!();
            assert!(result.ends_with("test_function"));
        }

        test_function();
    }

    #[test]
    fn test_get_data_dir() {
        let data_dir = super::get_data_dir().unwrap();
        assert_eq!(
            data_dir
                .components()
                .next_back()
                .unwrap()
                .as_os_str()
                .to_string_lossy(),
            "mecomp"
        );
    }

    #[test]
    fn test_get_config_dir() {
        let config_dir = super::get_config_dir().unwrap();
        assert_eq!(
            config_dir
                .components()
                .next_back()
                .unwrap()
                .as_os_str()
                .to_string_lossy(),
            "mecomp"
        );
    }
}
