use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Failed to open file: {0}")]
    FileOpenError(#[from] std::io::Error),
    #[error("Failed to decode audio: {0}")]
    DecodeError(#[from] rodio::decoder::DecoderError),
    #[error("Failed to resample audio: {0}")]
    ResampleError(#[from] rubato::ResampleError),
    #[error("Failed to create resampler: {0}")]
    ResamplerConstructionError(#[from] rubato::ResamplerConstructionError),
    #[error("Failure During Analysis: {0}")]
    AnalysisError(String),
    #[error("Samples are empty or too short")]
    EmptySamples,
    #[error("Audio Source length is unknown or infinite")]
    InfiniteAudioSource,
    #[error("Too many or too little features were provided at the end of the analysis")]
    InvalidFeaturesLen,
}

pub type AnalysisResult<T> = Result<T, AnalysisError>;

#[derive(Error, Debug)]
pub enum ClusteringError {
    #[error("Library too small to cluster")]
    SmallLibrary,
    #[error("Optimal k could not be found below k={0}")]
    OptimalKNotFound(usize),
}
