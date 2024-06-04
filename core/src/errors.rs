use mecomp_storage::errors::Error;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(#[from] Error),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Decoder error: {0}")]
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
            LibraryError::Decoder(e) => Self::Decoder(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_str_eq;
    use rstest::rstest;

    #[test]
    fn test_serializable_library_error() {
        let error = LibraryError::Database(Error::NoId);
        let serializable_error: SerializableLibraryError = error.into();
        assert_str_eq!(
            serializable_error.to_string(),
            "Database error: Item is missing an Id."
        );

        let error = LibraryError::IO(std::io::Error::new(std::io::ErrorKind::Other, "test"));
        let serializable_error: SerializableLibraryError = error.into();
        assert_str_eq!(serializable_error.to_string(), "IO error: test");

        let error = LibraryError::Decoder(rodio::decoder::DecoderError::DecodeError("test"));
        let serializable_error: SerializableLibraryError = error.into();
        assert_str_eq!(serializable_error.to_string(), "Decoder error: test");
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
