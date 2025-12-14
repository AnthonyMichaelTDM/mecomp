//! Module for handling creating vector embeddings of audio data using theoretically any ONNX model,
//! but specifically designed for use with the model in `models/audio_embedding_model.onnx`.

use crate::ResampledAudio;
use ort::session::RunOptions;
use ort::{session::Session, value::TensorRef};
use std::path::Path;

static MODEL_BYTES: &[u8] = include_bytes!("../models/audio_embedding_model.onnx");

const EMBEDDING_SIZE: i64 = 32;

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
    pub fn embed(&mut self, audio: &ResampledAudio) -> ort::Result<Vec<f32>> {
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

        Ok(embedding.to_vec())
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
    pub async fn embed_async(&mut self, audio: &ResampledAudio) -> ort::Result<Vec<f32>> {
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

        Ok(embedding.to_vec())
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
        assert_eq!(embedding.len(), 32);
    }
}
