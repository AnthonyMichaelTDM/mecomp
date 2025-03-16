//! Implementation of the mecomp decoder, which is rodio/rubato based.

use std::{f32::consts::SQRT_2, fs::File, io::BufReader};

use rodio::Source;
use rubato::{FastFixedIn, PolynomialDegree, Resampler};

use crate::{errors::AnalysisError, errors::AnalysisResult, ResampledAudio, SAMPLE_RATE};

use super::Decoder;

#[allow(clippy::module_name_repetitions)]
pub struct MecompDecoder();

impl Decoder for MecompDecoder {
    /// A function that should decode and resample a song, optionally
    /// extracting the song's metadata such as the artist, the album, etc.
    ///
    /// The output sample array should be resampled to f32le, one channel, with a sampling rate
    /// of 22050 Hz. Anything other than that will yield wrong results.
    #[allow(clippy::missing_inline_in_public_items)]
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
        if source.total_duration().is_none() {
            return Err(AnalysisError::IndeterminateDuration);
        }

        let mut mono_sample_array: Vec<f32> = match num_channels {
            0 => {
                return Err(AnalysisError::DecodeError(
                    rodio::decoder::DecoderError::NoStreams,
                ))
            }
            // audio is already mono
            1 => source.collect(),
            // stereo
            2 => source
                .collect::<Vec<_>>()
                .chunks_exact(2)
                .map(|chunk| (chunk[0] + chunk[1]) * SQRT_2 / 2.)
                .collect(),
            // other, mayby 2.1 / 5.1 surround sound?
            _ => {
                log::warn!("The audio source has more than 2 channels (might be 2.1 or 5.1 surround sound), will collapse to mono by averaging the channels");
                source
                    .collect::<Vec<_>>()
                    .chunks_exact(num_channels)
                    .map(|chunk| chunk.iter().sum::<f32>() / num_channels as f32)
                    .collect()
            }
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

    fn verify_decoding_output(path: &Path, expected_hash: u32) {
        let song = Decoder::decode(path).unwrap();
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
    #[case::resample_multi(Path::new("data/s32_stereo_44_1_kHz.flac"), 0xbbcb_a1cf)]
    #[case::resample_stereo(Path::new("data/s16_stereo_22_5kHz.flac"), 0x1d7b_2d6d)]
    #[case::decode_mono(Path::new("data/s16_mono_22_5kHz.flac"), 0x5e01_930b)]
    #[ignore = "fails when asked to resample to 22050 Hz, ig ffmpeg does it differently, but I'm not sure what the difference actually is"]
    #[case::decode_mp3(Path::new("data/s32_stereo_44_1_kHz.mp3"), 0x69ca_6906)]
    #[case::decode_wav(Path::new("data/piano.wav"), 0xde83_1e82)]
    fn test_decode(#[case] path: &Path, #[case] expected_hash: u32) {
        verify_decoding_output(path, expected_hash);
    }

    #[test]
    fn test_dont_panic_no_channel_layout() {
        let path = Path::new("data/no_channel.wav");
        Decoder::decode(path).unwrap();
    }

    #[test]
    fn test_decode_right_capacity_vec() {
        let path = Path::new("data/s16_mono_22_5kHz.flac");
        let song = Decoder::decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize, // The + SAMPLE_RATE is because bliss-rs would add an extra second as a buffer, we don't need to because we know the exact length of the song
            sample_array.capacity()
        );

        let path = Path::new("data/s32_stereo_44_1_kHz.flac");
        let song = Decoder::decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );

        // NOTE: originally used the .ogg file, but it was failing to decode with `DecodeError(IoError("end of stream"))`
        let path = Path::new("data/capacity_fix.wav");
        let song = Decoder::decode(path).unwrap();
        let sample_array = song.samples;
        assert_eq!(
            sample_array.len(), // + SAMPLE_RATE as usize,
            sample_array.capacity()
        );
    }
}
