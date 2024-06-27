use mecomp_storage::errors::Error;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur with finding the config or data directories.
#[derive(Error, Debug)]
pub enum DirectoryError {
    #[error("Unable to find the config directory for mecomp.")]
    Config,
    #[error("Unable to find the data directory for mecomp.")]
    Data,
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
}

impl From<Error> for SerializableLibraryError {
    fn from(e: Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<std::io::Error> for SerializableLibraryError {
    fn from(e: std::io::Error) -> Self {
        Self::IO(e.to_string())
    }
}

#[cfg(feature = "audio")]
impl From<rodio::decoder::DecoderError> for SerializableLibraryError {
    fn from(e: rodio::decoder::DecoderError) -> Self {
        Self::Decoder(e.to_string())
    }
}

impl From<LibraryError> for SerializableLibraryError {
    fn from(e: LibraryError) -> Self {
        match e {
            LibraryError::Database(e) => Self::Database(e.to_string()),
            LibraryError::IO(e) => Self::IO(e.to_string()),
            #[cfg(feature = "audio")]
            LibraryError::Decoder(e) => Self::Decoder(e.to_string()),
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
        assert_str_eq!(SerializableLibraryError::from(input).to_string(), expected);
    }

    #[rstest]
    #[case(Error::NoId, LibraryError::Database(Error::NoId).into())]
    #[case(std::io::Error::new(std::io::ErrorKind::Other, "test"), LibraryError::IO(std::io::Error::new(std::io::ErrorKind::Other, "test")).into())]
    #[case(rodio::decoder::DecoderError::DecodeError("test"), LibraryError::Decoder(rodio::decoder::DecoderError::DecodeError("test")).into())]
    fn test_serializable_library_error_from<T: Into<SerializableLibraryError>>(
        #[case] from: T,
        #[case] to: SerializableLibraryError,
    ) {
        assert_eq!(from.into(), to);
    }
}
