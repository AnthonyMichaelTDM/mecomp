//! Implementation of the mecomp decoder, which is rodio/rubato based.

use std::{f32::consts::SQRT_2, fs::File, num::NonZeroUsize, time::Duration};

use object_pool::Pool;
use rubato::{FastFixedIn, Resampler, ResamplerConstructionError};
use symphonia::{
    core::{
        audio::{AudioBufferRef, SampleBuffer, SignalSpec},
        codecs::{DecoderOptions, CODEC_TYPE_NULL},
        errors::Error,
        formats::{FormatOptions, FormatReader},
        io::{MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        probe::Hint,
        units,
    },
    default::get_probe,
};

use crate::{errors::AnalysisError, errors::AnalysisResult, ResampledAudio, SAMPLE_RATE};

use super::Decoder;

const MAX_DECODE_RETRIES: usize = 3;
const CHUNK_SIZE: usize = 4096;

/// Struct used by the symphonia-based bliss decoders to decode audio files
#[doc(hidden)]
pub struct SymphoniaSource {
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    current_span_offset: usize,
    format: Box<dyn FormatReader>,
    total_duration: Option<Duration>,
    buffer: SampleBuffer<f32>,
    spec: SignalSpec,
}

impl SymphoniaSource {
    /// Create a new `SymphoniaSource` from a `MediaSourceStream`
    ///
    /// # Errors
    ///
    /// This function will return an error if the `MediaSourceStream` does not contain any streams, or if the stream
    /// is not supported by the decoder.
    pub fn new(mss: MediaSourceStream) -> Result<Self, Error> {
        Self::init(mss)?.ok_or(Error::DecodeError("No Streams"))
    }

    fn init(mss: MediaSourceStream) -> symphonia::core::errors::Result<Option<Self>> {
        let hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let mut probed_format = get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)?
            .format;

        let Some(stream) = probed_format.default_track() else {
            return Ok(None);
        };

        // Select the first supported track
        let track = probed_format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(Error::Unsupported("No track with supported codec"))?;

        let track_id = track.id;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        let total_duration = stream
            .codec_params
            .time_base
            .zip(stream.codec_params.n_frames)
            .map(|(base, spans)| base.calc_time(spans).into());

        let mut decode_errors: usize = 0;
        let decoded_audio = loop {
            let current_span = probed_format.next_packet()?;

            // If the packet does not belong to the selected track, skip over it
            if current_span.track_id() != track_id {
                continue;
            }

            match decoder.decode(&current_span) {
                Ok(audio) => break audio,
                Err(Error::DecodeError(_)) if decode_errors < MAX_DECODE_RETRIES => {
                    decode_errors += 1;
                }
                Err(e) => return Err(e),
            }
        };

        let spec = decoded_audio.spec().to_owned();
        let buffer = Self::get_buffer(decoded_audio, spec);
        Ok(Some(Self {
            decoder,
            current_span_offset: 0,
            format: probed_format,
            total_duration,
            buffer,
            spec,
        }))
    }

    #[inline]
    fn get_buffer(decoded: AudioBufferRef, spec: SignalSpec) -> SampleBuffer<f32> {
        let duration = units::Duration::from(decoded.capacity() as u64);
        let mut buffer = SampleBuffer::<f32>::new(duration, spec);
        buffer.copy_interleaved_ref(decoded);
        buffer
    }

    #[inline]
    #[must_use]
    pub const fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    #[inline]
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.spec.rate
    }

    #[inline]
    #[must_use]
    pub fn channels(&self) -> usize {
        self.spec.channels.count()
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.buffer.samples().len(),
            self.total_duration.map(|dur| {
                (usize::try_from(dur.as_secs()).unwrap_or(usize::MAX) + 1)
                    * self.spec.rate as usize
                    * self.spec.channels.count()
            }),
        )
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_span_offset < self.buffer.len() {
            let sample = self.buffer.samples().get(self.current_span_offset);
            self.current_span_offset += 1;

            return sample.copied();
        }

        let mut decode_errors = 0;
        let decoded = loop {
            let packet = self.format.next_packet().ok()?;
            match self.decoder.decode(&packet) {
                // Loop until we get a packet with audio frames. This is necessary because some
                // formats can have packets with only metadata, particularly when rewinding, in
                // which case the iterator would otherwise end with `None`.
                // Note: checking `decoded.frames()` is more reliable than `packet.dur()`, which
                // can returns non-zero durations for packets without audio frames.
                Ok(decoded) if decoded.frames() > 0 => break decoded,
                Ok(_) => {}
                Err(Error::DecodeError(_)) if decode_errors < MAX_DECODE_RETRIES => {
                    decode_errors += 1;
                }
                Err(_) => return None,
            }
        };

        decoded.spec().clone_into(&mut self.spec);
        self.buffer = Self::get_buffer(decoded, self.spec);
        self.current_span_offset = 1;
        self.buffer.samples().first().copied()
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct MecompDecoder<R = FastFixedIn<f32>> {
    resampler: Pool<Result<R, ResamplerConstructionError>>,
}

impl MecompDecoder {
    #[inline]
    fn generate_resampler() -> Result<FastFixedIn<f32>, ResamplerConstructionError> {
        FastFixedIn::new(1.0, 10.0, rubato::PolynomialDegree::Cubic, CHUNK_SIZE, 1)
    }

    /// Create a new `MecompDecoder`
    ///
    /// # Errors
    ///
    /// This function will return an error if the resampler could not be created.
    #[inline]
    pub fn new() -> Result<Self, AnalysisError> {
        // try to generate a resampler first, so we can return an error if it fails (if it fails, it's likely all future calls will too)
        let first = Self::generate_resampler()?;

        let pool_size = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
        let resampler = Pool::new(pool_size, Self::generate_resampler);
        resampler.attach(Ok(first));

        Ok(Self { resampler })
    }

    /// we need to collapse the audio source into one channel
    /// channels are interleaved, so if we have 2 channels, `[1, 2, 3, 4]` and `[5, 6, 7, 8]`,
    /// they will be stored as `[1, 5, 2, 6, 3, 7, 4, 8]`
    ///
    /// For stereo sound, we can make this mono by averaging the channels and multiplying by the square root of 2,
    /// This recovers the exact behavior of ffmpeg when converting stereo to mono, however for 2.1 and 5.1 surround sound,
    /// ffmpeg might be doing something different, and I'm not sure what that is (don't have a 5.1 surround sound file to test with)
    ///
    /// TODO: Figure out how ffmpeg does it for 2.1 and 5.1 surround sound, and do it the same way
    #[inline]
    #[doc(hidden)]
    pub fn into_mono_samples(
        source: Vec<f32>,
        num_channels: usize,
    ) -> Result<Vec<f32>, AnalysisError> {
        match num_channels {
            // no channels
            0 => Err(AnalysisError::DecodeError(Error::DecodeError(
                "The audio source has no channels",
            ))),
            // mono
            1 => Ok(source),
            // stereo
            2 => Ok(source
                .chunks_exact(2)
                .map(|chunk| (chunk[0] + chunk[1]) * SQRT_2 / 2.)
                .collect()),
            // 2.1 or 5.1 surround
            _ => {
                log::warn!("The audio source has more than 2 channels (might be 2.1 or 5.1 surround sound), will collapse to mono by averaging the channels");

                #[allow(clippy::cast_precision_loss)]
                let num_channels_f32 = num_channels as f32;
                let mono_samples = source
                    .chunks_exact(num_channels)
                    .map(|chunk| chunk.iter().sum::<f32>() / num_channels_f32)
                    .collect();

                Ok(mono_samples)
            }
        }
    }

    /// Resample the given mono samples to 22050 Hz
    #[inline]
    #[doc(hidden)]
    pub fn resample_mono_samples(
        &self,
        mut samples: Vec<f32>,
        sample_rate: u32,
        total_duration: Duration,
    ) -> Result<Vec<f32>, AnalysisError> {
        if sample_rate == SAMPLE_RATE {
            samples.shrink_to_fit();
            return Ok(samples);
        }

        let mut resampled_frames = Vec::with_capacity(
            (usize::try_from(total_duration.as_secs()).unwrap_or(usize::MAX) + 1)
                * SAMPLE_RATE as usize,
        );

        let (pool, resampler) = self.resampler.pull(Self::generate_resampler).detach();
        let mut resampler = resampler?;
        resampler.set_resample_ratio(f64::from(SAMPLE_RATE) / f64::from(sample_rate), false)?;

        let delay = resampler.output_delay();

        let new_length = samples.len() * SAMPLE_RATE as usize / sample_rate as usize;
        let mut output_buffer = resampler.output_buffer_allocate(true);

        // chunks of frames, each being CHUNKSIZE long.
        let sample_chunks = samples.chunks_exact(CHUNK_SIZE);
        let remainder = sample_chunks.remainder();

        for chunk in sample_chunks {
            debug_assert!(resampler.input_frames_next() == CHUNK_SIZE);

            let (_, output_written) =
                resampler.process_into_buffer(&[chunk], output_buffer.as_mut_slice(), None)?;
            resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
        }

        // process the remainder
        if !remainder.is_empty() {
            let (_, output_written) = resampler.process_partial_into_buffer(
                Some(&[remainder]),
                output_buffer.as_mut_slice(),
                None,
            )?;
            resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
        }

        // flush final samples from resampler
        if resampled_frames.len() < new_length + delay {
            let (_, output_written) = resampler.process_partial_into_buffer(
                Option::<&[&[f32]]>::None,
                output_buffer.as_mut_slice(),
                None,
            )?;
            resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
        }

        resampler.reset();
        pool.attach(Ok(resampler));

        Ok(resampled_frames[delay..new_length + delay].to_vec())
    }
}

impl Decoder for MecompDecoder {
    /// A function that should decode and resample a song, optionally
    /// extracting the song's metadata such as the artist, the album, etc.
    ///
    /// The output sample array should be resampled to f32le, one channel, with a sampling rate
    /// of 22050 Hz. Anything other than that will yield wrong results.
    #[allow(clippy::missing_inline_in_public_items)]
    fn decode(&self, path: &std::path::Path) -> AnalysisResult<ResampledAudio> {
        // open the file
        let file = File::open(path)?;
        // create the media source stream
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

        let source = SymphoniaSource::new(mss)?;

        // Convert the audio source into a mono channel
        let sample_rate = source.spec.rate;
        let Some(total_duration) = source.total_duration else {
            return Err(AnalysisError::IndeterminantDuration);
        };
        let num_channels = source.channels();

        let mono_sample_array =
            Self::into_mono_samples(source.into_iter().collect(), num_channels)?;

        // then we need to resample the audio source into 22050 Hz
        let resampled_array =
            self.resample_mono_samples(mono_sample_array, sample_rate, total_duration)?;

        Ok(ResampledAudio {
            path: path.to_owned(),
            samples: resampled_array,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::NUMBER_FEATURES;

    use super::{Decoder as DecoderTrait, MecompDecoder as Decoder};
    use adler32::RollingAdler32;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use std::path::Path;

    fn verify_decoding_output(path: &Path, expected_hash: u32) {
        let decoder = Decoder::new().unwrap();
        let song = decoder.decode(path).unwrap();
        let mut hasher = RollingAdler32::new();
        for sample in &song.samples {
            hasher.update_buffer(&sample.to_le_bytes());
        }

        assert_eq!(expected_hash, hasher.hash());
    }

    // expected hash Obtained through
    // ffmpeg -i data/s16_stereo_22_5kHz.flac -ar 22050 -ac 1 -c:a pcm_f32le -f hash -hash adler32 -
    #[rstest]
    #[ignore = "fails when asked to resample to 22050 Hz, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::resample_stereo(Path::new("data/s32_stereo_44_1_kHz.flac"), 0xbbcb_a1cf)]
    #[ignore = "fails when asked to resample to 22050 Hz, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::resample_mono(Path::new("data/s32_mono_44_1_kHz.flac"), 0xa0f8b8af)]
    #[case::decode_stereo(Path::new("data/s16_stereo_22_5kHz.flac"), 0x1d7b_2d6d)]
    #[case::decode_mono(Path::new("data/s16_mono_22_5kHz.flac"), 0x5e01_930b)]
    #[ignore = "fails when asked to resample to 22050 Hz, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::resample_mp3(Path::new("data/s32_stereo_44_1_kHz.mp3"), 0x69ca_6906)]
    #[case::decode_wav(Path::new("data/piano.wav"), 0xde83_1e82)]
    fn test_decode(#[case] path: &Path, #[case] expected_hash: u32) {
        verify_decoding_output(path, expected_hash);
    }

    #[test]
    fn test_dont_panic_no_channel_layout() {
        let path = Path::new("data/no_channel.wav");
        Decoder::new().unwrap().decode(path).unwrap();
    }

    #[test]
    fn test_decode_right_capacity_vec() {
        let path = Path::new("data/s16_mono_22_5kHz.flac");
        let song = Decoder::new().unwrap().decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize, // The + SAMPLE_RATE is because bliss-rs would add an extra second as a buffer, we don't need to because we know the exact length of the song
            sample_array.capacity()
        );

        let path = Path::new("data/s32_stereo_44_1_kHz.flac");
        let song = Decoder::new().unwrap().decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );

        // NOTE: originally used the .ogg file, but it was failing to decode with `DecodeError(IoError("end of stream"))`
        let path = Path::new("data/capacity_fix.wav");
        let song = Decoder::new().unwrap().decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );
    }

    const PATH_AND_EXPECTED_ANALYSIS: (&str, [f64; NUMBER_FEATURES]) = (
        "data/s16_mono_22_5kHz.flac",
        [
            0.3846389,
            -0.849141,
            -0.75481045,
            -0.8790748,
            -0.63258266,
            -0.7258959,
            -0.775738,
            -0.8146726,
            0.2716726,
            0.25779057,
            -0.35661936,
            -0.63578653,
            -0.29593682,
            0.06421304,
            0.21852458,
            -0.581239,
            -0.9466835,
            -0.9481153,
            -0.9820945,
            -0.95968974,
        ],
    );

    #[test]
    fn test_analyze() {
        let (path, expected_analysis) = PATH_AND_EXPECTED_ANALYSIS;
        let analysis = Decoder::new()
            .unwrap()
            .analyze_path(Path::new(path))
            .unwrap();
        for (x, y) in analysis.as_vec().iter().zip(expected_analysis) {
            assert!(0.01 > (x - y).abs());
        }
    }
}
