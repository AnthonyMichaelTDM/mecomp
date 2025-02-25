//! Miscellaneous feature extraction module.
//!
//! Contains various descriptors that don't fit in one of the
//! existing categories.

use bliss_audio_aubio_rs::level_lin;
use ndarray::{arr1, Axis};

use crate::Feature;

use super::utils::{mean, Normalize};

/**
 * Loudness (in dB) detection object.
 *
 * It indicates how "loud" a recording of a song is. For a given audio signal,
 * this value increases if the amplitude of the signal, and nothing else, is
 * increased.
 *
 * Of course, this makes this result dependent of the recording, meaning
 * the same song would yield different loudness on different recordings. Which
 * is exactly what we want, given that this is not a music theory project, but
 * one that aims at giving the best real-life results.
 *
 * Ranges between -90 dB (~silence) and 0 dB.
 *
 * (This is technically the sound pressure level of the track, but loudness is
 * way more visual)
 */
#[derive(Default, Clone)]
pub struct LoudnessDesc {
    pub values: Vec<f32>,
}

impl LoudnessDesc {
    pub const WINDOW_SIZE: usize = 1024;

    #[inline]
    pub fn do_(&mut self, chunk: &[f32]) {
        let level = level_lin(chunk);
        self.values.push(level);
    }

    #[inline]
    pub fn get_value(&mut self) -> Vec<Feature> {
        // Make sure the dB don't go less than -90dB
        let std_value = Feature::from(
            arr1(&self.values)
                .std_axis(Axis(0), 0.)
                .into_scalar()
                .max(1e-9),
        );
        let mean_value = Feature::from(mean(&self.values).max(1e-9));
        vec![
            self.normalize(10.0 * mean_value.log10()),
            self.normalize(10.0 * std_value.log10()),
        ]
    }
}

impl Normalize for LoudnessDesc {
    const MAX_VALUE: Feature = 0.;
    const MIN_VALUE: Feature = -90.;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder};
    use std::path::Path;

    #[test]
    fn test_loudness() {
        let song = Decoder::decode(Path::new("data/s16_mono_22_5kHz.flac")).unwrap();
        let mut loudness_desc = LoudnessDesc::default();
        for chunk in song.samples.chunks_exact(LoudnessDesc::WINDOW_SIZE) {
            loudness_desc.do_(chunk);
        }
        let expected_values = [0.271_263, 0.257_718_1];
        for (expected, actual) in expected_values.iter().zip(loudness_desc.get_value().iter()) {
            assert!(0.01 > (expected - actual).abs(), "{expected} !~= {actual}");
        }
    }

    #[test]
    fn test_loudness_boundaries() {
        let mut loudness_desc = LoudnessDesc::default();
        let silence_chunk = vec![0.; 1024];
        loudness_desc.do_(&silence_chunk);
        let expected_values = [-1., -1.];
        for (expected, actual) in expected_values.iter().zip(loudness_desc.get_value().iter()) {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }

        let mut loudness_desc = LoudnessDesc::default();
        let silence_chunk = vec![1.; 1024];
        loudness_desc.do_(&silence_chunk);
        let expected_values = [1., -1.];
        for (expected, actual) in expected_values.iter().zip(loudness_desc.get_value().iter()) {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }

        let mut loudness_desc = LoudnessDesc::default();
        let silence_chunk = vec![-1.; 1024];
        loudness_desc.do_(&silence_chunk);
        let expected_values = [1., -1.];
        for (expected, actual) in expected_values.iter().zip(loudness_desc.get_value().iter()) {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }
}
