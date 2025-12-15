//! Module for handling creating vector embeddings of audio data using theoretically any ONNX model,
//! but specifically designed for use with the model in `models/audio_embedding_model.onnx`.

use crate::ResampledAudio;
use ort::session::RunOptions;
use ort::{session::Session, value::TensorRef};
use std::path::Path;

static MODEL_BYTES: &[u8] = include_bytes!("../models/audio_embedding_model.onnx");

/// The size of the embedding produced by the audio embedding model, onnx wants this to be i64.
const EMBEDDING_SIZE: i64 = 32;
/// The size of the embedding produced by the audio embedding model as a usize.
pub const DIM_EMBEDDING: usize = 32;

#[derive(Default, PartialEq, Clone, Copy)]
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
}

/// Struct representing an audio embedding model loaded from an ONNX file.
pub struct AudioEmbeddingModel {
    session: ort::session::Session,
}

impl AudioEmbeddingModel {
    /// Load the default audio embedding model included in the package.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_default() -> ort::Result<Self> {
        let session = Session::builder()?
            .with_memory_pattern(false)?
            .commit_from_memory(MODEL_BYTES)?;

        Ok(Self { session })
    }

    /// Load an audio embedding model from the specified ONNX file path.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_from_onnx<P: AsRef<Path>>(path: P) -> ort::Result<Self> {
        let session = Session::builder()?
            .with_memory_pattern(false)?
            .commit_from_file(path)?;

        Ok(Self { session })
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
            "audio" => TensorRef::from_array_view(([1,audio.samples.len()], &*audio.samples))?,
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

    /// Compute embedding from raw audio samples (f32, mono, 22050 Hz),
    /// runs asynchronously.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// * the audio cannot be converted into a tensor,
    /// * the model inference fails,
    /// * the output is missing or has an unexpected shape (should be named "embedding" and have shape `[1, 32]`).
    #[inline]
    pub async fn embed_async(&mut self, audio: &ResampledAudio) -> ort::Result<Embedding> {
        // Create input with batch dimension
        let inputs = ort::inputs! {
            "audio" => TensorRef::from_array_view(([1,audio.samples.len()], &*audio.samples))?,
        };

        // Run inference
        let options = RunOptions::new()?;
        let outputs = self.session.run_async(inputs, &options)?.await?;

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
}
