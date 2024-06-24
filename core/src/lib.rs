use errors::DirectoryError;

#[cfg(feature = "audio")]
pub mod audio;
pub mod errors;
pub mod logger;
#[cfg(feature = "rpc")]
pub mod rpc;
pub mod state;
#[cfg(test)]
pub mod test_utils;

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
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // `3` is the length of the `::f`.
        &name[..name.len() - 3]
    }};
}

#[must_use]
pub fn format_duration(duration: &std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = duration.as_secs_f32() % 60.;

    format!("{hours:02}:{minutes:02}:{seconds:05.2}")
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
        assert_eq!(format_duration(&duration), expected);
    }
}

/// Get the data directory for the application.
///
/// Follows the XDG Base Directory Specification for linux, and the equivalents on other platforms.
/// See the [`directories`](https://docs.rs/directories/latest/directories/) crate for more information.
///
/// # Errors
///
/// This function will return an error if the data directory could not be found.
pub fn get_data_dir() -> Result<std::path::PathBuf, DirectoryError> {
    let directory = if let Ok(s) = std::env::var("MECOMP_DATA") {
        std::path::PathBuf::from(s)
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "", "mecomp") {
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
pub fn get_config_dir() -> Result<std::path::PathBuf, DirectoryError> {
    let directory = if let Ok(s) = std::env::var("MECOMP_CONFIG") {
        std::path::PathBuf::from(s)
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "", "mecomp") {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        return Err(DirectoryError::Config);
    };
    Ok(directory)
}
