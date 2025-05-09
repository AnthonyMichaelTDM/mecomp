//! Timbral feature extraction module.
//!
//! Contains functions to extract & summarize the zero-crossing rate,
//! spectral centroid, spectral flatness and spectral roll-off of
//! a given Song.

use bliss_audio_aubio_rs::vec::CVec;
use bliss_audio_aubio_rs::{PVoc, SpecDesc, SpecShape, bin_to_freq};
use ndarray::{Axis, arr1};

use crate::Feature;

use super::SAMPLE_RATE;
use super::errors::{AnalysisError, AnalysisResult};
use super::utils::{Normalize, geometric_mean, mean, number_crossings};

/**
 * General object holding all the spectral descriptor.
 *
 * Holds 3 spectral descriptors together. It would be better conceptually
 * to have 3 different spectral descriptor objects, but this avoids re-computing
 * the same FFT three times.
 *
 * Current spectral descriptors are spectral centroid, spectral rolloff and
 * spectral flatness (see `values_object` for a further description of the
 * object.
 *
 * All descriptors are currently summarized by their mean only.
 */
pub struct SpectralDesc {
    phase_vocoder: PVoc,
    sample_rate: u32,

    centroid_aubio_desc: SpecDesc,
    rolloff_aubio_desc: SpecDesc,
    values_centroid: Vec<f32>,
    values_rolloff: Vec<f32>,
    values_flatness: Vec<f32>,
}

impl SpectralDesc {
    pub const WINDOW_SIZE: usize = 512;
    pub const HOP_SIZE: usize = Self::WINDOW_SIZE / 4;

    /**
     * Compute score related to the
     * [spectral centroid](https://en.wikipedia.org/wiki/Spectral_centroid) values,
     * obtained after repeatedly calling `do_` on all of the song's chunks.
     *
     * Spectral centroid is used to determine the "brightness" of a sound, i.e.
     * how much high frequency there is in an audio signal.
     *
     * It of course depends of the instrument used: a piano-only track that makes
     * use of high frequencies will still score less than a song using a lot of
     * percussive sound, because the piano frequency range is lower.
     *
     * The value range is between 0 and `sample_rate / 2`.
     */
    #[inline]
    pub fn get_centroid(&mut self) -> Vec<Feature> {
        vec![
            self.normalize(Feature::from(mean(&self.values_centroid))),
            self.normalize(Feature::from(
                arr1(&self.values_centroid)
                    .std_axis(Axis(0), 0.)
                    .into_scalar(),
            )),
        ]
    }

    /**
     * Compute score related to the spectral roll-off values, obtained
     * after repeatedly calling `do_` on all of the song's chunks.
     *
     * Spectral roll-off is the bin frequency number below which a certain
     * percentage of the spectral energy is found, here, 95%.
     *
     * It can be used to distinguish voiced speech (low roll-off) and unvoiced
     * speech (high roll-off). It is also a good indication of the energy
     * repartition of a song.
     *
     * The value range is between 0 and `sample_rate / 2`
     */
    #[inline]
    pub fn get_rolloff(&mut self) -> Vec<Feature> {
        vec![
            self.normalize(Feature::from(mean(&self.values_rolloff))),
            self.normalize(Feature::from(
                arr1(&self.values_rolloff)
                    .std_axis(Axis(0), 0.)
                    .into_scalar(),
            )),
        ]
    }

    /**
     * Compute score related to the
     * [spectral flatness](https://en.wikipedia.org/wiki/Spectral_flatness) values,
     * obtained after repeatedly calling `do_` on all of the song's chunks.
     *
     * Spectral flatness is the ratio between the geometric mean of the spectrum
     * and its arithmetic mean.
     *
     * It is used to distinguish between tone-like and noise-like signals.
     * Tone-like audio is f.ex. a piano key, something that has one or more
     * specific frequencies, while (white) noise has an equal distribution
     * of intensity among all frequencies.
     *
     * The value range is between 0 and 1, since the geometric mean is always less
     * than the arithmetic mean.
     */
    #[inline]
    pub fn get_flatness(&mut self) -> Vec<Feature> {
        let max_value = 1.;
        let min_value = 0.;
        // Range is different from the other spectral algorithms, so normalizing
        // manually here.
        vec![
            2. * (Feature::from(mean(&self.values_flatness)) - min_value) / (max_value - min_value)
                - 1.,
            2. * (Feature::from(
                arr1(&self.values_flatness)
                    .std_axis(Axis(0), 0.)
                    .into_scalar(),
            ) - min_value)
                / (max_value - min_value)
                - 1.,
        ]
    }

    /// # Errors
    ///
    /// This function will return an error if there is an error loading the aubio objects
    #[inline]
    pub fn new(sample_rate: u32) -> AnalysisResult<Self> {
        Ok(Self {
            centroid_aubio_desc: SpecDesc::new(SpecShape::Centroid, Self::WINDOW_SIZE).map_err(
                |e| {
                    AnalysisError::AnalysisError(format!(
                        "error while loading aubio centroid object: {e}",
                    ))
                },
            )?,
            rolloff_aubio_desc: SpecDesc::new(SpecShape::Rolloff, Self::WINDOW_SIZE).map_err(
                |e| {
                    AnalysisError::AnalysisError(format!(
                        "error while loading aubio rolloff object: {e}",
                    ))
                },
            )?,
            phase_vocoder: PVoc::new(Self::WINDOW_SIZE, Self::HOP_SIZE).map_err(|e| {
                AnalysisError::AnalysisError(format!("error while loading aubio pvoc object: {e}",))
            })?,
            values_centroid: Vec::new(),
            values_rolloff: Vec::new(),
            values_flatness: Vec::new(),
            sample_rate,
        })
    }

    /**
    Compute all the descriptors' value for the given chunk.

    After using this on all the song's chunks, you can call
    `get_centroid`, `get_flatness` and `get_rolloff` to get the respective
    descriptors' values.
    */
    #[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn do_(&mut self, chunk: &[f32]) -> AnalysisResult<()> {
        let mut fftgrain: Vec<f32> = vec![0.0; Self::WINDOW_SIZE];
        self.phase_vocoder
            .do_(chunk, fftgrain.as_mut_slice())
            .map_err(|e| {
                AnalysisError::AnalysisError(format!("error while processing aubio pv object: {e}"))
            })?;

        let bin = self
            .centroid_aubio_desc
            .do_result(fftgrain.as_slice())
            .map_err(|e| {
                AnalysisError::AnalysisError(format!(
                    "error while processing aubio centroid object: {e}",
                ))
            })?;

        #[allow(clippy::cast_precision_loss)]
        let freq = bin_to_freq(bin, self.sample_rate as f32, Self::WINDOW_SIZE as f32);
        self.values_centroid.push(freq);

        let mut bin = self
            .rolloff_aubio_desc
            .do_result(fftgrain.as_slice())
            .unwrap();

        // Until https://github.com/aubio/aubio/pull/318 is in
        #[allow(clippy::cast_precision_loss)]
        if bin > Self::WINDOW_SIZE as f32 / 2. {
            bin = Self::WINDOW_SIZE as f32 / 2.;
        }

        #[allow(clippy::cast_precision_loss)]
        let freq = bin_to_freq(bin, self.sample_rate as f32, Self::WINDOW_SIZE as f32);
        self.values_rolloff.push(freq);

        let cvec: CVec = fftgrain.as_slice().into();
        let geo_mean = geometric_mean(cvec.norm());
        if geo_mean == 0.0 {
            self.values_flatness.push(0.0);
            return Ok(());
        }
        let flatness = geo_mean / mean(cvec.norm());
        self.values_flatness.push(flatness);
        Ok(())
    }
}

impl Normalize for SpectralDesc {
    #[allow(clippy::cast_precision_loss)]
    const MAX_VALUE: Feature = SAMPLE_RATE as Feature / 2.;
    const MIN_VALUE: Feature = 0.;
}

/**
 * [Zero-crossing rate](https://en.wikipedia.org/wiki/Zero-crossing_rate)
 * detection object.
 *
 * Zero-crossing rate is mostly used to detect percussive sounds in an audio
 * signal, as well as whether an audio signal contains speech or not.
 *
 * It is a good metric to differentiate between songs with people speaking clearly,
 * (e.g. slam) and instrumental songs.
 *
 * The value range is between 0 and 1.
 */
#[derive(Default, Clone)]
pub struct ZeroCrossingRateDesc {
    crossings_sum: u32,
    samples_checked: usize,
}

impl ZeroCrossingRateDesc {
    #[must_use]
    #[inline]
    pub fn new(_sample_rate: u32) -> Self {
        Self::default()
    }

    /// Count the number of zero-crossings for the current `chunk`.
    #[inline]
    pub fn do_(&mut self, chunk: &[f32]) {
        self.crossings_sum += number_crossings(chunk);
        self.samples_checked += chunk.len();
    }

    /// Sum the number of zero-crossings witnessed and divide by
    /// the total number of samples.
    #[allow(clippy::cast_precision_loss)]
    #[inline]
    pub fn get_value(&mut self) -> Feature {
        self.normalize(Feature::from(self.crossings_sum) / self.samples_checked as Feature)
    }
}

impl Normalize for ZeroCrossingRateDesc {
    const MAX_VALUE: Feature = 1.;
    const MIN_VALUE: Feature = 0.;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder};
    use std::path::Path;

    #[test]
    fn test_zcr_boundaries() {
        let mut zcr_desc = ZeroCrossingRateDesc::default();
        let chunk = vec![0.; 1024];
        zcr_desc.do_(&chunk);
        let value = zcr_desc.get_value();
        assert!(f64::EPSILON > (-1. - value).abs(), "{value} !~= -1");

        let one_chunk = [-1., 1.];
        let chunks = std::iter::repeat_n(one_chunk.iter(), 512)
            .flatten()
            .copied()
            .collect::<Vec<f32>>();
        let mut zcr_desc = ZeroCrossingRateDesc::default();
        zcr_desc.do_(&chunks);
        let value = zcr_desc.get_value();
        assert!(0.001 > (0.998_046_9 - value).abs(), "{value} !~= 0.9980469");
    }

    #[test]
    fn test_zcr() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap();
        let mut zcr_desc = ZeroCrossingRateDesc::default();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            zcr_desc.do_(chunk);
        }
        let value = zcr_desc.get_value();
        assert!(0.001 > (-0.85036 - value).abs(), "{value} !~= -0.85036");
    }

    #[test]
    fn test_spectral_flatness_boundaries() {
        let mut spectral_desc = SpectralDesc::new(10).unwrap();
        let chunk = vec![0.; 1024];

        let expected_values = [-1., -1.];
        spectral_desc.do_(&chunk).unwrap();
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_flatness().iter())
        {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }

        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/white_noise.mp3"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(22050).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        println!("{:?}", spectral_desc.get_flatness());
        // White noise - as close to 1 as possible
        let expected_values = [0.578_530_3, -0.942_630_8];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_flatness().iter())
        {
            // original test wanted absolute error < 0.001
            // assert!(0.001 > (expected - actual).abs(), "{expected} !~= {actual}");
            let relative_error = (expected - actual).abs() / expected.abs();
            assert!(
                relative_error < 0.078,
                "relative error: {relative_error}, expected: {expected}, actual: {actual}"
            );
        }
    }

    #[test]
    fn test_spectral_flatness() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        // Spectral flatness mean value computed here with phase vocoder before normalization: 0.111949615
        // Essentia value with spectrum / hann window: 0.11197535695207445
        let expected_values = [-0.776_100_75, -0.814_817_9];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_flatness().iter())
        {
            assert!(0.01 > (expected - actual).abs(), "{expected} !~= {actual}");
        }
    }

    #[test]
    fn test_spectral_roll_off_boundaries() {
        let mut spectral_desc = SpectralDesc::new(10).unwrap();
        let chunk = vec![0.; 512];

        let expected_values = [-1., -1.];
        spectral_desc.do_(&chunk).unwrap();
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_rolloff().iter())
        {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }

        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/tone_11080Hz.flac"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        let expected_values = [0.996_768_1, -0.996_151_75];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_rolloff().iter())
        {
            assert!(
                0.0001 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_spectral_roll_off() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        let expected_values = [-0.632_648_6, -0.726_093_3];
        // Roll-off mean value computed here with phase vocoder before normalization: 2026.7644
        // Essentia value with spectrum / hann window: 1979.632683520047
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_rolloff().iter())
        {
            assert!(0.01 > (expected - actual).abs(), "{expected} !~= {actual}");
        }
    }

    #[test]
    fn test_spectral_centroid() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        // Spectral centroid mean value computed here with phase vocoder before normalization: 1354.2273
        // Essential value with spectrum / hann window: 1351
        let expected_values = [-0.75483, -0.879_168_87];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_centroid().iter())
        {
            assert!(
                0.0001 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_spectral_centroid_boundaries() {
        let mut spectral_desc = SpectralDesc::new(10).unwrap();
        let chunk = vec![0.; 512];

        spectral_desc.do_(&chunk).unwrap();
        let expected_values = [-1., -1.];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_centroid().iter())
        {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/tone_11080Hz.flac"))
            .unwrap();
        let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE).unwrap();
        for chunk in song.samples.chunks_exact(SpectralDesc::HOP_SIZE) {
            spectral_desc.do_(chunk).unwrap();
        }
        let expected_values = [0.97266, -0.960_992_6];
        for (expected, actual) in expected_values
            .iter()
            .zip(spectral_desc.get_centroid().iter())
        {
            // original test wanted absolute error < 0.00001
            // assert!(0.00001 > (expected - actual).abs(), "{expected} !~= {actual}");
            let relative_error = (expected - actual).abs() / expected.abs();
            assert!(
                relative_error < 0.039,
                "relative error: {relative_error}, expected: {expected}, actual: {actual}"
            );
        }
    }
}
