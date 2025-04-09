#![allow(clippy::missing_inline_in_public_items)]

use std::{
    clone::Clone,
    marker::Send,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use log::info;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{errors::AnalysisResult, Analysis, ResampledAudio};

mod mecomp;
#[allow(clippy::module_name_repetitions)]
pub use mecomp::{MecompDecoder, SymphoniaSource};

/// Trait used to implement your own decoder.
///
/// The `decode` function should be implemented so that it
/// decodes and resample a song to one channel with a sampling rate of 22050 Hz
/// and a f32le layout.
/// Once it is implemented, several functions
/// to perform analysis from path(s) are available, such as
/// [`song_from_path`](Decoder::song_from_path) and
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
        self.decode(path.as_ref())?.try_into()
    }

    /// Analyze songs in `paths`, and return the `Analysis` objects through an
    /// [`mpsc::IntoIter`].
    ///
    /// Returns an iterator, whose items are a tuple made of
    /// the song path (to display to the user in case the analysis failed),
    /// and a `Result<Analysis>`.
    #[inline]
    fn analyze_paths<P: Into<PathBuf>, F: IntoIterator<Item = P>>(
        &self,
        paths: F,
    ) -> mpsc::IntoIter<(PathBuf, AnalysisResult<Analysis>)>
    where
        Self: Sync + Send,
    {
        let cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        self.analyze_paths_with_cores(paths, cores)
    }

    /// Analyze songs in `paths`, and return the `Analysis` objects through an
    /// [`mpsc::IntoIter`]. `number_cores` sets the number of cores the analysis
    /// will use, capped by your system's capacity. Most of the time, you want to
    /// use the simpler `analyze_paths` functions, which autodetects the number
    /// of cores in your system.
    ///
    /// Return an iterator, whose items are a tuple made of
    /// the song path (to display to the user in case the analysis failed),
    /// and a `Result<Analysis>`.
    fn analyze_paths_with_cores<P: Into<PathBuf>, F: IntoIterator<Item = P>>(
        &self,
        paths: F,
        number_cores: NonZeroUsize,
    ) -> mpsc::IntoIter<(PathBuf, AnalysisResult<Analysis>)>
    where
        Self: Sync + Send,
    {
        let (tx, rx) = mpsc::channel::<(PathBuf, AnalysisResult<Analysis>)>();
        self.analyze_paths_with_cores_with_callback(paths, number_cores, tx);
        rx.into_iter()
    }

    /// Returns a decoded song's `Analysis` given a file path, or an error if the song
    /// could not be analyzed for some reason.
    ///
    /// # Arguments
    ///
    /// * `path` - A [`Path`] holding a valid file path to a valid audio file.
    /// * `callback` - A function that will be called with the path and the result of the analysis.
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
    fn analyze_path_with_callback<P: AsRef<Path>>(
        &self,
        path: P,
        callback: mpsc::Sender<(P, AnalysisResult<Analysis>)>,
    ) {
        let song = self.analyze_path(&path);
        callback.send((path, song)).unwrap();

        // We don't need to return the result of the send, as the receiver will
    }

    /// Analyze songs in `paths`, and return the `Analysis` objects through an
    /// [`mpsc::IntoIter`].
    ///
    /// Returns an iterator, whose items are a tuple made of
    /// the song path (to display to the user in case the analysis failed),
    /// and a `Result<Analysis>`.
    #[inline]
    fn analyze_paths_with_callback<P: Into<PathBuf>, I: Send + IntoIterator<Item = P>>(
        &self,
        paths: I,
        callback: mpsc::Sender<(PathBuf, AnalysisResult<Analysis>)>,
    ) where
        Self: Sync + Send,
    {
        let cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        self.analyze_paths_with_cores_with_callback(paths, cores, callback);
    }

    /// Analyze songs in `paths`, and return the `Analysis` objects through an
    /// [`mpsc::IntoIter`]. `number_cores` sets the number of cores the analysis
    /// will use, capped by your system's capacity. Most of the time, you want to
    /// use the simpler `analyze_paths_with_callback` functions, which autodetects the number
    /// of cores in your system.
    ///
    /// Return an iterator, whose items are a tuple made of
    /// the song path (to display to the user in case the analysis failed),
    /// and a `Result<Analysis>`.
    fn analyze_paths_with_cores_with_callback<P: Into<PathBuf>, I: IntoIterator<Item = P>>(
        &self,
        paths: I,
        number_cores: NonZeroUsize,
        callback: mpsc::Sender<(PathBuf, AnalysisResult<Analysis>)>,
    ) where
        Self: Sync + Send,
    {
        let mut cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        if cores > number_cores {
            cores = number_cores;
        }
        let paths: Vec<PathBuf> = paths.into_iter().map(Into::into).collect();

        if paths.is_empty() {
            return;
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cores.get())
            .build()
            .unwrap();

        pool.install(|| {
            paths.into_par_iter().for_each(|path| {
                info!("Analyzing file '{path:?}'");
                self.analyze_path_with_callback(path, callback.clone());
            });
        });
    }
}
