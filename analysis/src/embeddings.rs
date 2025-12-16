//! Module for handling creating vector embeddings of audio data using theoretically any ONNX model,
//! but specifically designed for use with the model in `models/audio_embedding_model.onnx`.

use crate::ResampledAudio;
use log::warn;
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use std::path::{Path, PathBuf};

static MODEL_BYTES: &[u8] = include_bytes!("../models/audio_embedding_model.onnx");

/// The size of the embedding produced by the audio embedding model, onnx wants this to be i64.
const EMBEDDING_SIZE: i64 = 32;
/// The size of the embedding produced by the audio embedding model as a usize.
pub const DIM_EMBEDDING: usize = 32;

#[derive(Debug, Default, PartialEq, Clone, Copy)]
#[repr(transparent)]
pub struct Embedding(pub [f32; DIM_EMBEDDING]);

impl Embedding {
    /// Get the length of the embedding vector.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the embedding is empty.
    ///
    /// Should always return false since embeddings have a fixed size.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get a reference to the embedding as a slice.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// Get a mutable reference to the embedding as a slice.
    #[inline]
    #[must_use]
    pub const fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.0
    }

    /// Get the inner array of the embedding.
    #[inline]
    #[must_use]
    pub const fn inner(&self) -> &[f32; DIM_EMBEDDING] {
        &self.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    pub path: Option<PathBuf>,
}

/// Struct representing an audio embedding model loaded from an ONNX file.
#[derive(Debug)]
pub struct AudioEmbeddingModel {
    session: ort::session::Session,
}

fn session_builder() -> ort::Result<ort::session::builder::SessionBuilder> {
    let builder = Session::builder()?
        .with_memory_pattern(false)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?;

    Ok(builder)
}

impl AudioEmbeddingModel {
    /// Load the default audio embedding model included in the package.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_default() -> ort::Result<Self> {
        let session = session_builder()?.commit_from_memory(MODEL_BYTES)?;

        Ok(Self { session })
    }

    /// Load an audio embedding model from the specified ONNX file path.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_from_onnx<P: AsRef<Path>>(path: P) -> ort::Result<Self> {
        let session = session_builder()?.commit_from_file(&path)?;

        Ok(Self { session })
    }

    /// Load the an audio embedding model with the specified configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for how the model should be loaded.
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load(config: &ModelConfig) -> ort::Result<Self> {
        config.path.as_ref().map_or_else(Self::load_default, |path|{
            Self::load_from_onnx(path).or_else(|e| {
                warn!("failed to load embeddings model from specified path: {e}, falling back to default model.");
                Self::load_default()
            })

        })
    }

    /// Compute embedding from raw audio samples (f32, mono, 22050 Hz),
    /// blocks during execution.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// * the audio cannot be converted into a tensor,
    /// * the model inference fails,
    /// * the output is missing or has an unexpected shape (should be named "embedding" and have shape `[1, 32]`).
    #[inline]
    pub fn embed(&mut self, audio: &ResampledAudio) -> ort::Result<Embedding> {
        // Create input with batch dimension
        let inputs = ort::inputs! {
            "audio" => TensorRef::from_array_view(([1, audio.samples.len()], audio.samples.as_slice()))?,
        };

        // Run inference
        let outputs = self.session.run(inputs)?;

        // Extract embedding
        let (shape, embedding) = outputs["embedding"].try_extract_tensor::<f32>()?;

        let expected_shape = &[1, EMBEDDING_SIZE];
        if shape.iter().as_slice() != expected_shape {
            return Err(ort::Error::new(format!(
                "Unexpected embedding shape: {shape:?}, expected {expected_shape:?}",
            )));
        }

        let sized_embedding: [f32; DIM_EMBEDDING] = embedding
            .try_into()
            .map_err(|_| ort::Error::new("Failed to convert embedding to fixed-size array"))?;

        Ok(Embedding(sized_embedding))
    }

    /// Compute embeddings for a batch of raw audio samples (f32, mono, 22050 Hz),
    /// blocks during execution.
    ///
    /// For efficiency, all audio samples should be similar in length.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// * the audio cannot be converted into a tensor,
    /// * the model inference fails,
    /// * the output is missing or has an unexpected shape (should be named "embedding" and have shape `[batch_size, 32]`).
    #[inline]
    pub fn embed_batch(&mut self, audios: &[ResampledAudio]) -> ort::Result<Vec<Embedding>> {
        let max_len = audios.iter().map(|a| a.samples.len()).max().unwrap_or(0);

        let batch_size = audios.len();

        // Prepare input tensor with zero-padding
        let mut input_data = vec![0f32; batch_size * max_len];
        for (i, audio) in audios.iter().enumerate() {
            input_data[i * max_len..i * max_len + audio.samples.len()]
                .copy_from_slice(&audio.samples);
        }

        let input = ort::inputs! {
            "audio" => TensorRef::from_array_view(([batch_size, max_len], &*input_data))?,
        };

        // Run inference
        let outputs = self.session.run(input)?;

        // Extract embeddings
        let (shape, embedding_tensor) = outputs["embedding"].try_extract_tensor::<f32>()?;
        #[allow(clippy::cast_possible_wrap)]
        let expected_shape = &[batch_size as i64, EMBEDDING_SIZE];
        if shape.iter().as_slice() != expected_shape {
            return Err(ort::Error::new(format!(
                "Unexpected embedding shape: {shape:?}, expected {expected_shape:?}",
            )));
        }

        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let start = i * DIM_EMBEDDING;
            let end = start + DIM_EMBEDDING;
            let sized_embedding: [f32; DIM_EMBEDDING] = embedding_tensor[start..end]
                .try_into()
                .map_err(|_| ort::Error::new("Failed to convert embedding to fixed-size array"))?;
            embeddings.push(Embedding(sized_embedding));
        }

        Ok(embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::Decoder;
    use crate::decoder::MecompDecoder;

    const TEST_AUDIO_PATH: &str = "data/5_mins_of_noise_stereo_48kHz.ogg";

    #[test]
    fn test_embedding_model() {
        let decoder = MecompDecoder::new().unwrap();
        let audio = decoder
            .decode(Path::new(TEST_AUDIO_PATH))
            .expect("Failed to decode test audio");

        let mut model =
            AudioEmbeddingModel::load_default().expect("Failed to load embedding model");
        let embedding = model.embed(&audio).expect("Failed to compute embedding");
        assert_eq!(embedding.len(), DIM_EMBEDDING);
    }

    #[test]
    fn test_embedding_model_batch() {
        let decoder = MecompDecoder::new().unwrap();
        let audio = decoder
            .decode(Path::new(TEST_AUDIO_PATH))
            .expect("Failed to decode test audio");

        let audios = vec![audio.clone(); 4];

        let mut model =
            AudioEmbeddingModel::load_default().expect("Failed to load embedding model");
        let embeddings = model
            .embed_batch(&audios)
            .expect("Failed to compute batch embeddings");
        assert_eq!(embeddings.len(), 4);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), DIM_EMBEDDING);
        }

        // since all the audios are the same, all embeddings should be the same
        for embedding in &embeddings[1..] {
            assert_eq!(embedding, &embeddings[0]);
        }
    }

    #[test]
    fn test_embedding_model_batch_mixed_sizes() {
        let decoder = MecompDecoder::new().unwrap();
        let audio1 = decoder
            .decode(Path::new(TEST_AUDIO_PATH))
            .expect("Failed to decode test audio");

        // create a shorter audio by taking only the first half of the samples
        let audio2 = ResampledAudio {
            samples: audio1.samples[..audio1.samples.len() / 2].to_vec(),
            path: audio1.path.clone(),
        };

        let audios = vec![audio1.clone(), audio2.clone()];

        let mut model =
            AudioEmbeddingModel::load_default().expect("Failed to load embedding model");
        let embeddings = model
            .embed_batch(&audios)
            .expect("Failed to compute batch embeddings");
        assert_eq!(embeddings.len(), 2);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), DIM_EMBEDDING);
        }
    }
}
