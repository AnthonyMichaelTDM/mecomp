use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use log::info;

use crate::{errors::AnalysisResult, Analysis, ResampledAudio};

mod mecomp;
#[allow(clippy::module_name_repetitions)]
pub use mecomp::MecompDecoder;

/// Trait used to implement your own decoder.
///
/// The `decode` function should be implemented so that it
/// decodes and resample a song to one channel with a sampling rate of 22050 Hz
/// and a f32le layout.
/// Once it is implemented, several functions
/// to perform analysis from path(s) are available, such as
/// [`song_from_path`](Decoder::song_from_path) and
/// [`analyze_paths`](Decoder::analyze_paths).
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
    fn decode(path: &Path) -> AnalysisResult<ResampledAudio>;

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
    fn analyze_path<P: AsRef<Path>>(path: P) -> AnalysisResult<Analysis> {
        Self::decode(path.as_ref())?.try_into()
    }

    /// Analyze songs in `paths`, and return the `Analysis` objects through an
    /// [`mpsc::IntoIter`].
    ///
    /// Returns an iterator, whose items are a tuple made of
    /// the song path (to display to the user in case the analysis failed),
    /// and a `Result<Analysis>`.
    fn analyze_paths<P: Into<PathBuf>, F: IntoIterator<Item = P>>(
        paths: F,
    ) -> mpsc::IntoIter<(PathBuf, AnalysisResult<Analysis>)> {
        let cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        Self::analyze_paths_with_cores(paths, cores)
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
        paths: F,
        number_cores: NonZeroUsize,
    ) -> mpsc::IntoIter<(PathBuf, AnalysisResult<Analysis>)> {
        let mut cores = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
        if cores > number_cores {
            cores = number_cores;
        }
        let paths: Vec<PathBuf> = paths.into_iter().map(Into::into).collect();
        #[allow(clippy::type_complexity)]
        let (tx, rx): (
            mpsc::Sender<(PathBuf, AnalysisResult<Analysis>)>,
            mpsc::Receiver<(PathBuf, AnalysisResult<Analysis>)>,
        ) = mpsc::channel();
        if paths.is_empty() {
            return rx.into_iter();
        }
        let mut handles = Vec::new();
        let mut chunk_length = paths.len() / cores;
        if chunk_length == 0 {
            chunk_length = paths.len();
        }
        for chunk in paths.chunks(chunk_length) {
            let tx_thread = tx.clone();
            let owned_chunk = chunk.to_owned();
            let child = thread::spawn(move || {
                for path in owned_chunk {
                    info!("Analyzing file '{:?}'", path);
                    let song = Self::analyze_path(&path);
                    tx_thread.send((path.clone(), song)).unwrap();
                }
            });
            handles.push(child);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        rx.into_iter()
    }
}
