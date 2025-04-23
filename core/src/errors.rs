use std::{fmt::Debug, path::PathBuf};

use mecomp_storage::errors::Error;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error in the UDP stack.
#[derive(Error, Debug)]
#[cfg(feature = "rpc")]
pub enum UdpError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Ciborium deserialization error: {0}")]
    CiboriumDeserialization(#[from] ciborium::de::Error<std::io::Error>),
    #[error("Ciborium serialization error: {0}")]
    CiboriumSerialization(#[from] ciborium::ser::Error<std::io::Error>),
}

/// Errors that can occur with finding the config or data directories.
#[derive(Error, Debug)]
pub enum DirectoryError {
    #[error("Unable to find the config directory for mecomp.")]
    Config,
    #[error("Unable to find the data directory for mecomp.")]
    Data,
}

/// Errors that can occur when importing or exporting playlists or dynamic playlists
#[derive(Error, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum BackupError {
    #[error("The file \"{0}\" already exists")]
    FileExists(PathBuf),
    #[error("The file \"{0}\" does not exist")]
    FileNotFound(PathBuf),
    #[error("The file \"{0}\" has the wrong extension, expected: {1}")]
    WrongExtension(PathBuf, String),
    #[error("{0} is a directory, not a file")]
    PathIsDirectory(PathBuf),
    #[error("CSV error: {0}")]
    CsvError(String),
    #[error("IO Error: {0}")]
    IoError(String),
    #[error("Invalid playlist format")]
    InvalidDynamicPlaylistFormat,
    #[error("Error parsing dynamic playlist query in record {1}: {0}")]
    InvalidDynamicPlaylistQuery(String, usize),
    #[error("Error parsing playlist name in line {0}, name is empty or already set")]
    PlaylistNameInvalidOrAlreadySet(usize),
    #[error(
        "Out of {0} entries, no valid songs were found in the playlist, consult the logs for more information"
    )]
    NoValidSongs(usize),
    #[error("No valid playlists were found in the csv file.")]
    NoValidPlaylists,
}

impl From<csv::Error> for BackupError {
    #[inline]
    fn from(value: csv::Error) -> Self {
        Self::CsvError(format!("{value}"))
    }
}

impl From<std::io::Error> for BackupError {
    #[inline]
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

/// Errors that can occur with the library.
#[derive(Error, Debug)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(#[from] Error),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Decoder error: {0}")]
    #[cfg(feature = "audio")]
    Decoder(#[from] rodio::decoder::DecoderError),
    #[error("UdpError: {0}")]
    #[cfg(feature = "rpc")]
    Udp(#[from] UdpError),
}

#[derive(Error, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SerializableLibraryError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    IO(String),
    #[error("Decoder error: {0}")]
    Decoder(String),
    #[error("Library Rescan already in progress.")]
    RescanInProgress,
    #[error("Library Analysis already in progress.")]
    AnalysisInProgress,
    #[error("Collection Reclustering already in progress.")]
    ReclusterInProgress,
    #[error("UdpError: {0}")]
    #[cfg(feature = "rpc")]
    Udp(String),
    #[error("Backup Error: {0}")]
    BackupError(#[from] BackupError),
}

impl From<Error> for SerializableLibraryError {
    #[inline]
    fn from(e: Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<std::io::Error> for SerializableLibraryError {
    #[inline]
    fn from(e: std::io::Error) -> Self {
        Self::IO(e.to_string())
    }
}

#[cfg(feature = "audio")]
impl From<rodio::decoder::DecoderError> for SerializableLibraryError {
    #[inline]
    fn from(e: rodio::decoder::DecoderError) -> Self {
        Self::Decoder(e.to_string())
    }
}

#[cfg(feature = "rpc")]
impl From<UdpError> for SerializableLibraryError {
    #[inline]
    fn from(e: UdpError) -> Self {
        Self::Udp(e.to_string())
    }
}

impl From<LibraryError> for SerializableLibraryError {
    #[inline]
    fn from(e: LibraryError) -> Self {
        match e {
            LibraryError::Database(e) => Self::Database(e.to_string()),
            LibraryError::IO(e) => Self::IO(e.to_string()),
            #[cfg(feature = "audio")]
            LibraryError::Decoder(e) => Self::Decoder(e.to_string()),
            #[cfg(feature = "rpc")]
            LibraryError::Udp(e) => Self::Udp(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_str_eq;
    use rstest::rstest;

    #[rstest]
    #[case(
        LibraryError::from(Error::NoId),
        "Database error: Item is missing an Id."
    )]
    #[case(
        LibraryError::from(std::io::Error::new(std::io::ErrorKind::Other, "test")),
        "IO error: test"
    )]
    #[case(
        LibraryError::from(rodio::decoder::DecoderError::DecodeError("test")),
        "Decoder error: test"
    )]
    fn test_serializable_library_error(#[case] input: LibraryError, #[case] expected: String) {
        let actual = SerializableLibraryError::from(input).to_string();
        assert_str_eq!(actual, expected);
    }

    #[rstest]
    #[case(Error::NoId, LibraryError::Database(Error::NoId).into())]
    #[case(std::io::Error::new(std::io::ErrorKind::Other, "test"), LibraryError::IO(std::io::Error::new(std::io::ErrorKind::Other, "test")).into())]
    #[case(rodio::decoder::DecoderError::DecodeError("test"), LibraryError::Decoder(rodio::decoder::DecoderError::DecodeError("test")).into())]
    fn test_serializable_library_error_from<T: Into<SerializableLibraryError>>(
        #[case] from: T,
        #[case] to: SerializableLibraryError,
    ) {
        let actual = from.into();
        assert_eq!(actual, to);
    }
}
