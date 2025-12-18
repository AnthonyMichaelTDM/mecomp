#![allow(clippy::missing_inline_in_public_items)]

use std::{
    cell::RefCell,
    clone::Clone,
    marker::Send,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc::{self, SendError, SyncSender},
    thread,
};

use log::{debug, error, trace};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    Analysis, ResampledAudio,
    embeddings::{AudioEmbeddingModel, Embedding},
    errors::{AnalysisError, AnalysisResult},
};

mod mecomp;
#[allow(clippy::module_name_repetitions)]
pub use mecomp::{MecompDecoder, SymphoniaSource};

pub type ProcessingCallback =
    SyncSender<(PathBuf, AnalysisResult<Analysis>, AnalysisResult<Embedding>)>;

/// Trait used to implement your own decoder.
///
/// The `decode` function should be implemented so that it
/// decodes and resample a song to one channel with a sampling rate of 22050 Hz
/// and a f32le layout.
/// Once it is implemented, several functions
/// to perform analysis from path(s) are available, such as
/// [`analyze_paths_with_cores`](Decoder::analyze_paths_with_cores) and
/// [`analyze_paths`](Decoder::analyze_paths).
#[allow(clippy::module_name_repetitions)]
pub trait Decoder {
    /// A function that should decode and resample a song, optionally
    /// extracting the song's metadata such as the artist, the album, etc.
    ///
    /// The output sample array should be resampled to f32le, one channel, with a sampling rate
    /// of 22050 Hz. Anything other than that will yield wrong results.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file path is invalid, if
    /// the file path points to a file containing no or corrupted audio stream,
    /// or if the analysis could not be conducted to the end for some reason.
    ///
    /// The error type returned should give a hint as to whether it was a
    /// decoding or an analysis error.
    fn decode(&self, path: &Path) -> AnalysisResult<ResampledAudio>;

    /// Returns a decoded song's `Analysis` given a file path, or an error if the song
    /// could not be analyzed for some reason.
    ///
    /// see:
    /// - [`Decoder::decode`] for how songs are decoded
    /// - [`Analysis::from_samples`] for how analyses are calculated from [`ResampledAudio`]
    ///
    /// # Arguments
    ///
    /// * `path` - A [`Path`] holding a valid file path to a valid audio file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file path is invalid, if
    /// the file path points to a file containing no or corrupted audio stream,
    /// or if the analysis could not be conducted to the end for some reason.
    ///
    /// The error type returned should give a hint as to whether it was a
    /// decoding or an analysis error.
    #[inline]
    fn analyze_path<P: AsRef<Path>>(&self, path: P) -> AnalysisResult<Analysis> {
        Analysis::from_samples(&self.decode(path.as_ref())?)
    }

    /// Analyze songs in `paths` in parallel across all logical cores,
    /// and emits the [`AnalysisResult<Analysis>`] objects (along with the [`Path`] they correspond to)
    /// through the provided [callback channel](`mpsc::Sender`).
    ///
    /// This function is blocking, so it should be called in a separate thread
    /// from where the [receiver](`mpsc::Receiver`) is consumed.
    ///
    /// You can cancel the job by dropping the `callback` channel's [receiver](`mpsc::Receiver`).
    ///
    /// see [`Decoder::analyze_path`] for more details on how the analyses are generated.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mecomp_analysis::decoder::{Decoder as _, MecmopDecoder as Decoder};
    ///
    /// let paths = vec![
    ///     "data/piano.wav",
    ///     "data/s32_mono_44_1_kHz.flac"
    /// ];
    ///
    /// let (tx, rx) = std::mpsc::channel();
    ///
    /// let handle = std::thread::spawn(move || {
    ///     Decoder::new().unwrap().analyze_paths(paths, tx).unwrap();
    /// });
    ///
    /// for (path, maybe_analysis) = rx {
    ///     if let Ok(analysis) = maybe_analysis {
    ///         println!("{} analyzed successfully!", path.display());
    ///         // do something with the analysis
    ///     } else {
    ///         eprintln!("error analyzing {}!", path.display());
    ///     }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Errors if the `callback` channel is closed.
    #[inline]
    fn analyze_paths<P: Into<PathBuf>, I: Send + IntoIterator<Item = P>>(
        &self,
        paths: I,
        callback: mpsc::Sender<(PathBuf, AnalysisResult<Analysis>)>,
    ) -> Result<(), SendError<()>>
    where
        Self: Sync + Send,
    {
        let cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        self.analyze_paths_with_cores(paths, cores, callback)
    }

    /// Analyze songs in `paths` in parallel across `number_cores` threads,
    /// and emits the [`AnalysisResult<Analysis>`] objects (along with the [`Path`] they correspond to)
    /// through the provided [callback channel](`mpsc::Sender`).
    ///
    /// This function is blocking, so it should be called in a separate thread
    /// from where the [receiver](`mpsc::Receiver`) is consumed.
    ///
    /// You can cancel the job by dropping the `callback` channel's [receiver](`mpsc::Receiver`).
    ///
    /// See also: [`Decoder::analyze_paths`]
    ///
    /// # Errors
    ///
    /// Errors if the `callback` channel is closed.
    fn analyze_paths_with_cores<P: Into<PathBuf>, I: IntoIterator<Item = P>>(
        &self,
        paths: I,
        number_cores: NonZeroUsize,
        callback: mpsc::Sender<(PathBuf, AnalysisResult<Analysis>)>,
    ) -> Result<(), SendError<()>>
    where
        Self: Sync + Send,
    {
        let mut cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        if cores > number_cores {
            cores = number_cores;
        }
        let paths: Vec<PathBuf> = paths.into_iter().map(Into::into).collect();

        if paths.is_empty() {
            return Ok(());
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cores.get())
            .build()
            .unwrap();

        pool.install(|| {
            paths.into_par_iter().try_for_each(|path| {
                debug!("Analyzing file '{}'", path.display());
                let analysis = self.analyze_path(&path);
                callback.send((path, analysis)).map_err(|_| SendError(()))
            })
        })
    }

    /// Process raw audio samples in `audios`, and yield the `Analysis` and `Embedding` objects
    /// through the provided `callback` channel.
    /// Parallelizes the process across `number_cores` CPU cores.
    ///
    /// You can cancel the job by dropping the `callback` channel.
    ///
    /// Note: A new [`AudioEmbeddingModel`](crate::embeddings::AudioEmbeddingModel) session will be created
    /// for each batch processed.
    ///
    /// # Errors
    ///
    /// Errors if the `callback` channel is closed.
    #[inline]
    fn process_songs_with_cores(
        &self,
        paths: &[PathBuf],
        callback: ProcessingCallback,
        number_cores: NonZeroUsize,
        model_config: crate::embeddings::ModelConfig,
    ) -> AnalysisResult<()>
    where
        Self: Sync + Send,
    {
        let mut cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        if cores > number_cores {
            cores = number_cores;
        }

        if paths.is_empty() {
            return Ok(());
        }

        thread_local! {
            static MODEL: RefCell<Option<AudioEmbeddingModel>> = const { RefCell::new(None) };
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cores.get())
            .thread_name(|idx| format!("Analyzer {idx}"))
            .build()
            .unwrap();

        pool.install(|| {
            // Process songs in parallel, but each song is fully processed (decode -> analyze -> embed -> send)
            // before the thread moves to the next song. This prevents memory accumulation from
            // buffering decoded audio across multiple pipeline stages.
            paths.into_par_iter().try_for_each(|path| {
                let thread_name = thread::current().name().unwrap_or("unknown").to_string();

                // Decode the audio file
                let audio = match self.decode(path) {
                    Ok(audio) => {
                        trace!("Decoded {} in thread {thread_name}", path.display());
                        audio
                    }
                    Err(e) => {
                        error!("Error decoding {}: {e}", path.display());
                        return Ok(()); // Skip this file, continue with others
                    }
                };

                // Analyze the audio
                let analysis = Analysis::from_samples(&audio);
                trace!("Analyzed {} in thread {thread_name}", path.display());

                // Generate embedding
                let embedding = MODEL.try_with(|model_cell| {
                    let mut model_ref = model_cell.borrow_mut();
                    if model_ref.is_none() {
                        debug!("Loading embedding model in thread {thread_name}");
                        *model_ref = Some(AudioEmbeddingModel::load(&model_config)?);
                    }
                    trace!(
                        "Generating embeddings for {} in thread {thread_name}",
                        path.display()
                    );
                    model_ref
                        .as_mut()
                        .unwrap()
                        .embed(&audio)
                        .map_err(AnalysisError::from)
                });

                // Flatten the Result<Result<...>> and convert to AnalysisResult
                let embedding = match embedding {
                    Ok(Ok(e)) => Ok(e),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(AnalysisError::AccessError(e)),
                };

                // Drop the audio samples before sending to free memory immediately
                drop(audio);

                // Send the results - the bounded channel will apply backpressure
                // if the consumer is slow, preventing unbounded memory growth
                callback
                    .send((path.clone(), analysis, embedding))
                    .map_err(|_| AnalysisError::SendError)
            })
        })
    }

    /// Process raw audio samples in `audios`, and yield the `Analysis` and `Embedding` objects
    /// through the provided `callback` channel.
    /// Parallelizes the process across all available CPU cores.
    ///
    /// You can cancel the job by dropping the `callback` channel.
    ///
    /// Note: A new [`AudioEmbeddingModel`](crate::embeddings::AudioEmbeddingModel) session will be created
    /// for each batch processed.
    ///
    /// # Errors
    /// Errors if the `callback` channel is closed.
    #[inline]
    fn process_songs(
        &self,
        paths: &[PathBuf],
        callback: ProcessingCallback,
        model_config: crate::embeddings::ModelConfig,
    ) -> AnalysisResult<()>
    where
        Self: Sync + Send,
    {
        let cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        self.process_songs_with_cores(paths, callback, cores, model_config)
    }
}
