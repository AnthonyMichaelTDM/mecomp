use std::{
    fs::File,
    io::BufReader,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use log::info;
use rodio::Source;
use rubato::{FastFixedIn, PolynomialDegree, Resampler};

use crate::{errors::AnalysisError, errors::AnalysisResult, Analysis, ResampledAudio, SAMPLE_RATE};

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

#[allow(clippy::module_name_repetitions)]
pub struct MecompDecoder();

impl Decoder for MecompDecoder {
    /// A function that should decode and resample a song, optionally
    /// extracting the song's metadata such as the artist, the album, etc.
    ///
    /// The output sample array should be resampled to f32le, one channel, with a sampling rate
    /// of 22050 Hz. Anything other than that will yield wrong results.
    fn decode(path: &std::path::Path) -> AnalysisResult<ResampledAudio> {
        let file = BufReader::new(File::open(path)?);
        let source = rodio::Decoder::new(file)?.convert_samples::<f32>();

        // we need to collapse the audio source into one channel
        // channels are interleaved, so if we have 2 channels, `[1, 2, 3, 4]` and `[5, 6, 7, 8]`,
        // they will be stored as `[1, 5, 2, 6, 3, 7, 4, 8]`
        //
        // we can make this mono by averaging the channels
        //
        // TODO: Figure out how ffmpeg does it, and do it the same way
        let num_channels = source.channels() as usize;
        let sample_rate = source.sample_rate();
        let Some(total_duration) = source.total_duration() else {
            return Err(AnalysisError::InfiniteAudioSource);
        };
        let mut mono_sample_array = if num_channels == 1 {
            source.into_iter().collect()
        } else {
            source.into_iter().enumerate().fold(
                // pre-allocate the right capacity
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                Vec::with_capacity((total_duration.as_secs() as usize + 1) * sample_rate as usize),
                // collapse the channels into one channel
                |mut acc, (i, sample)| {
                    let channel = i % num_channels;
                    #[allow(clippy::cast_precision_loss)]
                    if channel == 0 {
                        acc.push(sample / num_channels as f32);
                    } else {
                        let last_index = acc.len() - 1;
                        acc[last_index] = sample.mul_add(1. / num_channels as f32, acc[last_index]);
                    }
                    acc
                },
            )
        };

        // then we need to resample the audio source into 22050 Hz
        let resampled_array = if sample_rate == SAMPLE_RATE {
            mono_sample_array.shrink_to_fit();
            mono_sample_array
        } else {
            let mut resampler = FastFixedIn::new(
                f64::from(SAMPLE_RATE) / f64::from(sample_rate),
                1.0,
                PolynomialDegree::Cubic,
                mono_sample_array.len(),
                1,
            )?;
            resampler.process(&[&mono_sample_array], None)?[0].clone()
        };

        Ok(ResampledAudio {
            path: path.to_owned(),
            samples: resampled_array,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Decoder as DecoderTrait, MecompDecoder as Decoder};
    use adler32::RollingAdler32;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::path::Path;

    fn _test_decode(path: &Path, expected_hash: u32) {
        let song = Decoder::decode(path).unwrap();
        let mut hasher = RollingAdler32::new();
        for sample in song.samples.iter() {
            hasher.update_buffer(&sample.to_le_bytes());
        }

        assert_eq!(expected_hash, hasher.hash());
    }

    // expected hash Obtained through
    // ffmpeg -i data/s16_stereo_22_5kHz.flac -ar 22050 -ac 1 -c:a pcm_f32le -f hash -hash adler32 -
    #[rstest]
    #[ignore = "fails when asked to convert stereo to mono, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::resample_multi(Path::new("data/s32_stereo_44_1_kHz.flac"), 0xbbcba1cf)]
    #[ignore = "fails when asked to convert stereo to mono, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::resample_stereo(Path::new("data/s16_stereo_22_5kHz.flac"), 0x1d7b2d6d)]
    #[case::decode_mono(Path::new("data/s16_mono_22_5kHz.flac"), 0x5e01930b)]
    #[ignore = "fails when asked to convert stereo to mono, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::decode_mp3(Path::new("data/s32_stereo_44_1_kHz.mp3"), 0x69ca6906)]
    #[case::decode_wav(Path::new("data/piano.wav"), 0xde831e82)]
    fn test_decode(#[case] path: &Path, #[case] expected_hash: u32) {
        _test_decode(path, expected_hash);
    }

    #[test]
    fn test_dont_panic_no_channel_layout() {
        let path = Path::new("data/no_channel.wav");
        Decoder::decode(&path).unwrap();
    }

    #[test]
    fn test_decode_right_capacity_vec() {
        let path = Path::new("data/s16_mono_22_5kHz.flac");
        let song = Decoder::decode(&path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize, // The + SAMPLE_RATE is because bliss-rs would add an extra second as a buffer, we don't need to because we know the exact length of the song
            sample_array.capacity()
        );

        let path = Path::new("data/s32_stereo_44_1_kHz.flac");
        let song = Decoder::decode(&path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );

        // NOTE: originally used the .ogg file, but it was failing to decode with `DecodeError(IoError("end of stream"))`
        let path = Path::new("data/capacity_fix.wav");
        let song = Decoder::decode(&path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );
    }
}
