//! Module for handling creating vector embeddings of audio data using theoretically any ONNX model,
//! but specifically designed for use with the model in `models/audio_embedding_model.onnx`.

use crate::ResampledAudio;
use log::warn;
use ort::{
    environment::Environment,
    ep,
    session::{
        Session,
        builder::{GraphOptimizationLevel, SessionBuilder},
    },
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
    session: Session,
}

/// Register a list of execution providers in priority order based on available features/platform.
/// Providers are tried in order; if one fails to register, the next is attempted.
/// CPU is always the final fallback.
///
/// # Errors
///
/// Returns an error if the CPU EP failed to register, ignores other EP registration failures
fn register_execution_providers(builder: &mut SessionBuilder) -> ort::Result<()> {
    // #[allow(unused_macros)]
    // macro_rules! register_ep {
    //     ($ep:expr, $builder:expr, $name:expr) => {
    //         if let Err(e) = $ep.build().register($builder) {
    //             log::error!("Failed to register {} execution provider: {:?}", $name, e);
    //         } else {
    //             log::info!("Successfully registered {} execution provider", $name);
    //         }
    //     };
    // }

    // let ep = ep::WebGPU::default();
    // register_ep!(ep, builder, "WebGPU");

    // CPU is always the final fallback
    // Disable the CPU memory arena to prevent memory accumulation when processing
    // variable-length audio inputs. The arena allocator grows to accommodate the
    // largest input seen but never shrinks, which causes memory to accumulate
    // when processing many songs of varying lengths.
    ep::CPU::default()
        .with_arena_allocator(false)
        .build()
        .register(builder)?;

    Ok(())
}

fn session_builder(environment: &Environment) -> ort::Result<SessionBuilder> {
    let mut builder = Session::builder(environment)?
        .with_memory_pattern(false)?
        .with_approximate_gelu()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?;

    // register execution providers
    register_execution_providers(&mut builder)?;

    Ok(builder)
}

impl AudioEmbeddingModel {
    /// Create a new environment to use to spawn sessions.
    /// # Panics
    ///
    /// Panics if the environment cannot be created for some reason.
    #[inline]
    pub fn default_environment() -> Environment {
        static ENV: std::sync::OnceLock<Environment> = std::sync::OnceLock::new();
        ENV.get_or_init(|| {
            ort::init()
                .with_name("AudioEmbeddingModel")
                .build()
                .expect("Failed to initialize environment")
        });
        ENV.get().unwrap().clone()
    }

    /// Load the default audio embedding model included in the package.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_default(env: &Environment) -> ort::Result<Self> {
        let session = session_builder(env)?.commit_from_memory(MODEL_BYTES)?;

        Ok(Self { session })
    }

    /// Load an audio embedding model from the specified ONNX file path.
    ///
    /// # Errors
    /// Fails if the model cannot be loaded for some reason.
    #[inline]
    pub fn load_from_onnx<P: AsRef<Path>>(env: &Environment, path: P) -> ort::Result<Self> {
        let session = session_builder(env)?.commit_from_file(&path)?;

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
    pub fn load(env: &Environment, config: &ModelConfig) -> ort::Result<Self> {
        config.path.as_ref().map_or_else(| | Self::load_default(env), |path|{
            Self::load_from_onnx(env, path).or_else(|e| {
                warn!("failed to load embeddings model from specified path: {e}, falling back to default model.");
                Self::load_default(env)
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
    #[allow(clippy::missing_inline_in_public_items)]
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
    #[allow(clippy::missing_inline_in_public_items)]
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
    use rstest::rstest;

    use super::*;
    use crate::decoder::Decoder;
    use crate::decoder::MecompDecoder;

    const TEST_AUDIO_PATH: &str = "data/5_mins_of_noise_stereo_48kHz.ogg";

    #[rstest::fixture]
    fn env() -> ort::environment::Environment {
        AudioEmbeddingModel::default_environment()
    }

    #[rstest]
    fn test_embedding_model(env: ort::environment::Environment) {
        let decoder = MecompDecoder::new().unwrap();
        let audio = decoder
            .decode(Path::new(TEST_AUDIO_PATH))
            .expect("Failed to decode test audio");

        let mut model =
            AudioEmbeddingModel::load_default(&env).expect("Failed to load embedding model");
        let embedding = model.embed(&audio).expect("Failed to compute embedding");
        assert_eq!(embedding.len(), DIM_EMBEDDING);
    }

    #[rstest]
    fn test_embedding_model_batch(env: ort::environment::Environment) {
        let decoder = MecompDecoder::new().unwrap();
        let audio = decoder
            .decode(Path::new(TEST_AUDIO_PATH))
            .expect("Failed to decode test audio");

        let audios = vec![audio.clone(); 4];

        let mut model =
            AudioEmbeddingModel::load_default(&env).expect("Failed to load embedding model");
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

    #[rstest]
    fn test_embedding_model_batch_mixed_sizes(env: ort::environment::Environment) {
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
            AudioEmbeddingModel::load_default(&env).expect("Failed to load embedding model");
        let embeddings = model
            .embed_batch(&audios)
            .expect("Failed to compute batch embeddings");
        assert_eq!(embeddings.len(), 2);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), DIM_EMBEDDING);
        }
    }
}
