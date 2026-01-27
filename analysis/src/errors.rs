use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Failed to open file: {0}")]
    FileOpenError(#[from] std::io::Error),
    #[error("Failed to decode audio: {0}")]
    DecodeError(#[from] symphonia::core::errors::Error),
    #[error("Failed to resample audio: {0}")]
    ResampleError(#[from] rubato::ResampleError),
    #[error("Failed to create resampler: {0}")]
    ResamplerConstructionError(#[from] rubato::ResamplerConstructionError),
    #[error("Failure During Analysis: {0}")]
    AnalysisError(String),
    #[error("Samples are empty or too short")]
    EmptySamples,
    #[error("Audio Source length is unknown or infinite")]
    IndeterminantDuration,
    #[error("Too many or too little features were provided at the end of the analysis")]
    InvalidFeaturesLen,
    #[error("Embedding Error: {0}")]
    EmbeddingError(#[from] ort::Error),
    #[error("Send Error")]
    SendError,
    #[error("Access Error when accessing thread-local model: {0}")]
    AccessError(#[from] std::thread::AccessError),
}

pub type AnalysisResult<T> = Result<T, AnalysisError>;

#[derive(Error, Debug)]
pub enum ClusteringError {
    #[error("Error during GMM clustering: {0}")]
    Gmm(#[from] linfa_clustering::GmmError),
    #[error("Error during KMeans clustering: {0}")]
    KMeans(#[from] linfa_clustering::KMeansError),
    #[error("Library too small to cluster")]
    SmallLibrary,
    #[error("Optimal k could not be found below k={0}")]
    OptimalKNotFound(usize),
    #[error("Failed to project data {0}")]
    ProjectionError(#[from] ProjectionError),
}

#[derive(Error, Debug)]
pub enum ProjectionError {
    #[error("with T-SNE: {0}")]
    TSneError(#[from] linfa_tsne::TSneError),
    #[error("with PCA: {0}")]
    PcaError(#[from] linfa_reduction::ReductionError),
}
