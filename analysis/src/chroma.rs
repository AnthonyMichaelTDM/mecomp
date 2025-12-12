//! Chroma feature extraction module.
//!
//! Contains functions to compute the chromagram of a song, and
//! then from this chromagram extract the song's tone and mode
//! (minor / major).
use crate::Feature;

use super::errors::{AnalysisError, AnalysisResult};
use super::utils::{Normalize, hz_to_octs_inplace, stft};
use bitvec::vec::BitVec;
use likely_stable::{LikelyResult, likely, unlikely};
use ndarray::{Array, Array1, Array2, Axis, Order, Zip, arr1, arr2, concatenate, s};
use ndarray_stats::QuantileExt;
use ndarray_stats::interpolate::Midpoint;
use noisy_float::prelude::*;

/**
 * General object holding the chroma descriptor.
 *
 * Current chroma descriptors are interval features (see
 * <https://speech.di.uoa.gr/ICMC-SMC-2014/images/VOL_2/1461.pdf>).
 *
 * Contrary to the other descriptors that can be used with streaming
 * without consequences, this one performs better if the full song is used at
 * once.
 */
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct ChromaDesc {
    sample_rate: u32,
    n_chroma: u32,
    values_chroma: Array2<f64>,
}

impl Normalize for ChromaDesc {
    const MAX_VALUE: Feature = 1.0;
    const MIN_VALUE: Feature = 0.;
}

impl ChromaDesc {
    pub const WINDOW_SIZE: usize = 8192;
    /// The theoretical maximum value for IC1-6 is each value at (1/2)².
    /// The reason is that `extract_interval_features` computes the product of the
    /// L1-normalized chroma vector (so all of its values are <= 1) by itself.
    /// The maximum value of this is all coordinates to 1/2 (since dyads will
    /// select three values). The maximum of this is then (1/2)², so the maximum of its
    /// L2 norm is this sqrt(2 * (1/2)²) ~= 0.62. However, real-life simulations shown
    /// that 0.25 is a good ceiling value (see tests).
    pub const MAX_L2_INTERVAL: f64 = 0.25;
    /// The theoretical maximum value for IC7-10 is each value at (1/3)³.
    /// The reason is that `extract_interval_features` computes the product of the
    /// L1-normalized chroma vector (so all of its values are <= 1) by itself.
    /// The maximum value of this is all coordinates to 1/3 (since triads will
    /// select three values). The maximum of this is then (1/3)³, so the maximum of its
    /// L2 norm is this sqrt(4 * (1/3)³) ~= 0.074. However, real-life simulations shown
    /// that 0.025 is a good ceiling value (see tests).
    pub const MAX_L2_TRIAD: f64 = 0.025;
    /// We are using atan2 to keep the ratio bounded.
    pub const MAX_TRIAD_INTERVAL_RATIO: f64 = std::f64::consts::FRAC_PI_2;
    #[must_use]
    #[inline]
    pub fn new(sample_rate: u32, n_chroma: u32) -> Self {
        Self {
            sample_rate,
            n_chroma,
            values_chroma: Array2::zeros((n_chroma as usize, 0)),
        }
    }

    /**
     * Compute and store the chroma of a signal.
     *
     * Passing a full song here once instead of streaming smaller parts of the
     * song will greatly improve accuracy.
     */
    #[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
    #[inline]
    pub fn do_(&mut self, signal: &[f32]) -> AnalysisResult<()> {
        let stft = stft(signal, Self::WINDOW_SIZE, 2205);
        let tuning = estimate_tuning(self.sample_rate, &stft, Self::WINDOW_SIZE, 0.01, 12)?;
        let chroma = chroma_stft(
            self.sample_rate,
            &stft,
            Self::WINDOW_SIZE,
            self.n_chroma,
            tuning,
        )?;
        self.values_chroma = concatenate![Axis(1), self.values_chroma, chroma];
        Ok(())
    }

    /**
     * Get the song's interval features.
     *
     * Return the 6 pitch class set categories, as well as the major, minor,
     * diminished and augmented triads.
     *
     * See this paper <https://speech.di.uoa.gr/ICMC-SMC-2014/images/VOL_2/1461.pdf>
     * for more information ("Timbre-invariant Audio Features for Style Analysis of Classical
     * Music").
     */
    #[inline]
    pub fn get_value(&mut self) -> Vec<Feature> {
        let mut raw_features = chroma_interval_features(&self.values_chroma);
        let (mut interval_class, mut interval_class_mode) =
            raw_features.view_mut().split_at(Axis(0), 6);
        // Compute those two norms separately because the values for the IC1-6 and IC7-10 don't
        // have the same range.
        let l2_norm_interval_class = interval_class.dot(&interval_class).sqrt();
        let l2_norm_interval_class_mode = interval_class_mode.dot(&interval_class_mode).sqrt();
        if l2_norm_interval_class > 0. {
            interval_class /= l2_norm_interval_class;
        }
        if l2_norm_interval_class_mode > 0. {
            interval_class_mode /= l2_norm_interval_class_mode;
        }
        let mut features = raw_features.mapv_into_any(|x| self.normalize(x)).to_vec();

        let normalized_l2_norm_interval_class =
            (2. * (l2_norm_interval_class - 0.) / (Self::MAX_L2_INTERVAL - 0.) - 1.).min(1.);
        features.push(normalized_l2_norm_interval_class);
        let normalized_l2_norm_interval_class_mode =
            (2. * (l2_norm_interval_class_mode - 0.) / (Self::MAX_L2_TRIAD - 0.) - 1.).min(1.);
        features.push(normalized_l2_norm_interval_class_mode);
        let angle = (20. * l2_norm_interval_class_mode).atan2(l2_norm_interval_class + 1e-12_f64);
        let normalized_ratio = 2. * (angle - 0.) / (Self::MAX_TRIAD_INTERVAL_RATIO - 0.) - 1.;
        features.push(normalized_ratio);
        features
    }
}

// Functions below are Rust versions of python notebooks by AudioLabs Erlang
// (<https://www.audiolabs-erlangen.de/resources/MIR/FMP/C0/C0.html>)
#[allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]
#[must_use]
#[inline]
pub fn chroma_interval_features(chroma: &Array2<f64>) -> Array1<f64> {
    let chroma = normalize_feature_sequence(&(chroma * 15.).exp());
    let templates = arr2(&[
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 0, 1, 1, 0],
        [0, 0, 0, 1, 0, 0, 1, 0, 0, 1],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 1, 0, 0, 1, 0],
        [0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ]);
    let interval_feature_matrix = extract_interval_features(&chroma, &templates);
    interval_feature_matrix.mean_axis(Axis(1)).unwrap()
}

#[must_use]
#[inline]
pub fn extract_interval_features(chroma: &Array2<f64>, templates: &Array2<i32>) -> Array2<f64> {
    let mut f_intervals: Array2<f64> = Array::zeros((chroma.shape()[1], templates.shape()[1]));
    for (template, mut f_interval) in templates
        .axis_iter(Axis(1))
        .zip(f_intervals.axis_iter_mut(Axis(1)))
    {
        for shift in 0..12 {
            let mut vec: Vec<i32> = template.to_vec();
            vec.rotate_right(shift);
            let rolled = arr1(&vec);
            let power = Zip::from(chroma.t())
                .and_broadcast(&rolled)
                .map_collect(|&f, &s| f.powi(s))
                .map_axis_mut(Axis(1), |x| x.product());
            f_interval += &power;
        }
    }
    f_intervals.t().to_owned()
}

#[inline]
pub fn normalize_feature_sequence(feature: &Array2<f64>) -> Array2<f64> {
    let mut normalized_sequence = feature.to_owned();
    for mut column in normalized_sequence.columns_mut() {
        let sum: f64 = column.iter().copied().map(f64::abs).sum();
        if likely(sum >= 0.0001) {
            column /= sum;
        }
    }

    normalized_sequence
}

// All the functions below are more than heavily inspired from
// librosa"s code: https://github.com/librosa/librosa/blob/main/librosa/feature/spectral.py#L1165
// chroma(22050, n_fft=5, n_chroma=12)
//
// Could be precomputed, but it takes very little time to compute it
// on the fly compared to the rest of the functions, and we'd lose the
// possibility to tweak parameters.
#[allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::missing_inline_in_public_items
)]
pub fn chroma_filter(
    sample_rate: u32,
    n_fft: usize,
    n_chroma: u32,
    tuning: f64,
) -> AnalysisResult<Array2<f64>> {
    let ctroct = 5.0;
    let octwidth = 2.;
    let n_chroma_float = f64::from(n_chroma);
    let n_chroma2 = (n_chroma_float / 2.0).round();

    let frequencies = Array::linspace(0., f64::from(sample_rate), n_fft + 1);

    let mut freq_bins = frequencies;
    hz_to_octs_inplace(&mut freq_bins, tuning, n_chroma);
    freq_bins *= n_chroma_float;
    freq_bins[0] = 1.5f64.mul_add(-n_chroma_float, freq_bins[1]);

    let mut binwidth_bins = Array::ones(freq_bins.raw_dim());
    binwidth_bins
        .slice_mut(s![0..freq_bins.len() - 1])
        .assign(&(&freq_bins.slice(s![1..]) - &freq_bins.slice(s![..-1])).mapv(|x| x.max(1.)));

    let mut d: Array2<f64> = Array::zeros((n_chroma as usize, (freq_bins).len()));
    for (idx, mut row) in d.rows_mut().into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        row.fill(idx as f64);
    }
    d = -d + &freq_bins;

    d = d + n_chroma2 + 10. * n_chroma_float;
    d = d % n_chroma_float - n_chroma2;
    d = d / binwidth_bins;
    d = (-2. * d.pow2()).exp();

    let mut wts = d;
    // Normalize by computing the l2-norm over the columns
    for mut col in wts.columns_mut() {
        let sum = col.pow2().sum().sqrt();
        if sum >= f64::MIN_POSITIVE {
            col /= sum;
        }
    }

    // Apply Gaussian tuning curve
    freq_bins = (-0.5 * ((freq_bins / n_chroma_float - ctroct) / octwidth).powi(2)).exp();

    wts *= &freq_bins;

    // np.roll(), np bro
    let mut b = Array2::zeros(wts.dim());
    b.slice_mut(s![-3.., ..]).assign(&wts.slice(s![..3, ..]));
    b.slice_mut(s![..-3, ..]).assign(&wts.slice(s![3.., ..]));

    wts = b;
    let non_aliased = 1 + n_fft / 2;
    Ok(wts.slice_move(s![.., ..non_aliased]))
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#[allow(clippy::missing_inline_in_public_items)]
pub fn pip_track(
    sample_rate: u32,
    spectrum: &Array2<f64>,
    n_fft: usize,
) -> AnalysisResult<(Vec<f64>, Vec<f64>)> {
    let sample_rate_float = f64::from(sample_rate);
    let fmin = 150.0_f64;
    let fmax = 4000.0_f64.min(sample_rate_float / 2.0);
    let threshold = 0.1;

    let fft_freqs = Array::linspace(0., sample_rate_float / 2., 1 + n_fft / 2);

    let length = spectrum.len_of(Axis(0));

    let freq_mask = fft_freqs
        .iter()
        .map(|&f| (fmin <= f) && (f < fmax))
        .collect::<BitVec>();

    let ref_value = spectrum.map_axis(Axis(0), |x| {
        let first: f64 = *x.first().expect("empty spectrum axis");
        let max = x.fold(first, |acc, &elem| acc.max(elem));
        threshold * max
    });

    // compute number of taken columns and beginning / end indices
    let freq_mask_len = freq_mask.len();
    let (taken_columns, beginning, end) = freq_mask.iter().enumerate().fold(
        (0, freq_mask_len, 0),
        |(taken, beginning, end), (i, b)| {
            b.then(|| (taken + 1, beginning.min(i), end.max(i + 1)))
                .unwrap_or((taken, beginning, end))
        },
    );

    // Validate that a valid frequency range was found
    if beginning >= end {
        return Err(AnalysisError::AnalysisError(String::from(
            "in chroma: no valid frequency range found",
        )));
    }
    // There will be at most taken_columns * length elements in pitches / mags
    let mut pitches = Vec::with_capacity(taken_columns * length);
    let mut mags = Vec::with_capacity(taken_columns * length);

    let zipped = Zip::indexed(spectrum.slice(s![beginning..end - 3, ..]))
        .and(spectrum.slice(s![beginning + 1..end - 2, ..]))
        .and(spectrum.slice(s![beginning + 2..end - 1, ..]));

    // No need to handle the last column, since freq_mask[length - 1] is
    // always going to be `false` for 22.5kHz
    zipped.for_each(|(i, j), &before_elem, &elem, &after_elem| {
        if elem > ref_value[j] && after_elem <= elem && before_elem < elem {
            let avg = 0.5 * (after_elem - before_elem);
            let mut shift = 2f64.mul_add(elem, -after_elem - before_elem);
            if shift.abs() < f64::MIN_POSITIVE {
                shift += 1.;
            }
            shift = avg / shift;
            #[allow(clippy::cast_precision_loss)]
            pitches.push(((i + beginning + 1) as f64 + shift) * sample_rate_float / n_fft as f64);
            mags.push((0.5 * avg).mul_add(shift, elem));
        }
    });

    Ok((pitches, mags))
}

// Only use this with strictly positive `frequencies`.
#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#[inline]
pub fn pitch_tuning(
    frequencies: &mut Array1<f64>,
    resolution: f64,
    bins_per_octave: u32,
) -> AnalysisResult<f64> {
    if unlikely(frequencies.is_empty()) {
        return Ok(0.0);
    }
    hz_to_octs_inplace(frequencies, 0.0, 12);
    frequencies.mapv_inplace(|x| f64::from(bins_per_octave) * x % 1.0);

    // Put everything between -0.5 and 0.5.
    frequencies.mapv_inplace(|x| if x >= 0.5 { x - 1. } else { x });

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let indexes = ((frequencies.to_owned() - -0.5) / resolution).mapv(|x| x as usize);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut counts: Array1<usize> = Array::zeros(((0.5 - -0.5) / resolution) as usize);
    for &idx in &indexes {
        counts[idx] += 1;
    }
    let max_index = counts
        .argmax()
        .map_err_unlikely(|e| AnalysisError::AnalysisError(format!("in chroma: {e}")))?;

    // Return the bin with the most reoccurring frequency.
    #[allow(clippy::cast_precision_loss)]
    Ok((100. * resolution).mul_add(max_index as f64, -50.) / 100.)
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#[inline]
pub fn estimate_tuning(
    sample_rate: u32,
    spectrum: &Array2<f64>,
    n_fft: usize,
    resolution: f64,
    bins_per_octave: u32,
) -> AnalysisResult<f64> {
    let (pitch, mag) = pip_track(sample_rate, spectrum, n_fft)?;

    let (filtered_pitch, filtered_mag): (Vec<N64>, Vec<N64>) = pitch
        .iter()
        .zip(&mag)
        .filter(|&(&p, _)| p > 0.)
        .map(|(x, y)| (n64(*x), n64(*y)))
        .unzip();

    if unlikely(pitch.is_empty()) {
        return Ok(0.);
    }

    let threshold: N64 = Array::from(filtered_mag.clone())
        .quantile_axis_mut(Axis(0), n64(0.5), &Midpoint)
        .map_err_unlikely(|e| AnalysisError::AnalysisError(format!("in chroma: {e}")))?
        .into_scalar();
    let mut pitch = filtered_pitch
        .iter()
        .zip(&filtered_mag)
        .filter_map(|(&p, &m)| if m >= threshold { Some(p.into()) } else { None })
        .collect::<Array1<f64>>();
    pitch_tuning(&mut pitch, resolution, bins_per_octave)
}

#[allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]
#[inline]
pub fn chroma_stft(
    sample_rate: u32,
    spectrum: &Array2<f64>, // shape: (window_length / 2 + 1, signal.len().div_ceil(hop_length))
    n_fft: usize,
    n_chroma: u32,
    tuning: f64,
) -> AnalysisResult<Array2<f64>> {
    let mut raw_chroma = chroma_filter(sample_rate, n_fft, n_chroma, tuning)?;

    raw_chroma = raw_chroma.dot(&spectrum.pow2());

    // We want to maximize cache locality, and are iterating over columns,
    // so let's make sure our array is in column-major order.
    raw_chroma = raw_chroma
        .to_shape((raw_chroma.dim(), Order::ColumnMajor))
        .map_err_unlikely(|_| {
            AnalysisError::AnalysisError(String::from("in chroma: failed to reorder array"))
        })?
        .to_owned();

    Zip::from(raw_chroma.columns_mut()).for_each(|mut row| {
        let sum = row.sum(); // we know that our values are positive, so no need to use abs
        if sum >= f64::MIN_POSITIVE {
            row /= sum;
        }
    });

    Ok(raw_chroma)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        SAMPLE_RATE,
        decoder::{Decoder as _, MecompDecoder as Decoder},
        utils::stft,
    };
    use ndarray::{Array2, arr1, arr2};
    use ndarray_npy::ReadNpyExt as _;
    use std::{fs::File, path::Path};

    #[test]
    fn test_chroma_interval_features() {
        let file = File::open("data/chroma.npy").unwrap();
        let chroma = Array2::<f64>::read_npy(file).unwrap();
        let features = chroma_interval_features(&chroma);
        let expected_features = arr1(&[
            0.038_602_84,
            0.021_852_81,
            0.042_243_79,
            0.063_852_78,
            0.073_111_48,
            0.025_125_66,
            0.003_198_99,
            0.003_113_08,
            0.001_074_33,
            0.002_418_61,
        ]);
        for (expected, actual) in expected_features.iter().zip(&features) {
            assert!(
                0.000_000_01 > (expected - actual.abs()),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_extract_interval_features() {
        let file = File::open("data/chroma-interval.npy").unwrap();
        let chroma = Array2::<f64>::read_npy(file).unwrap();
        let templates = arr2(&[
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 1, 0, 0, 0, 0, 1, 1, 0],
            [0, 0, 0, 1, 0, 0, 1, 0, 0, 1],
            [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 1, 0, 0, 1, 0],
            [0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ]);

        let file = File::open("data/interval-feature-matrix.npy").unwrap();
        let expected_interval_features = Array2::<f64>::read_npy(file).unwrap();

        let interval_features = extract_interval_features(&chroma, &templates);
        for (expected, actual) in expected_interval_features
            .iter()
            .zip(interval_features.iter())
        {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_normalize_feature_sequence() {
        let array = arr2(&[[0.1, 0.3, 0.4], [1.1, 0.53, 1.01]]);
        let expected_array = arr2(&[
            [0.083_333_33, 0.361_445_78, 0.283_687_94],
            [0.916_666_67, 0.638_554_22, 0.716_312_06],
        ]);

        let normalized_array = normalize_feature_sequence(&array);

        assert!(!array.is_empty() && !expected_array.is_empty());

        for (expected, actual) in normalized_array.iter().zip(expected_array.iter()) {
            assert!(
                0.000_000_1 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_chroma_desc() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let expected_values = [
            -0.342_925_13,
            -0.628_034_23,
            -0.280_950_96,
            0.086_864_59,
            0.244_460_82,
            -0.572_325_7,
            0.232_920_65,
            0.199_811_46,
            -0.585_944_06,
            -0.067_842_96,
        ];
        for (expected, actual) in expected_values.iter().zip(chroma_desc.get_value().iter()) {
            // original test wanted absolute error < 0.0000001
            let relative_error = (expected - actual).abs() / expected.abs();
            assert!(
                relative_error < 0.01,
                "relative error: {relative_error}, expected: {expected}, actual: {actual}"
            );
        }
    }

    #[test]
    fn test_chroma_stft_decode() {
        let signal = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap()
            .samples;
        let stft = stft(&signal, 8192, 2205);

        let file = File::open("data/chroma.npy").unwrap();
        let expected_chroma = Array2::<f64>::read_npy(file).unwrap();

        let chroma = chroma_stft(22050, &stft, 8192, 12, -0.049_999_999_999_999_99).unwrap();

        assert!(!chroma.is_empty() && !expected_chroma.is_empty());

        for (expected, actual) in expected_chroma.iter().zip(chroma.iter()) {
            // original test wanted absolute error < 0.0000001
            let relative_error = (expected - actual).abs() / expected.abs();
            assert!(
                relative_error < 0.01,
                "relative error: {relative_error}, expected: {expected}, actual: {actual}"
            );
        }
    }

    #[test]
    fn test_estimate_tuning() {
        let file = File::open("data/spectrum-chroma.npy").unwrap();
        let arr = Array2::<f64>::read_npy(file).unwrap();

        let tuning = estimate_tuning(22050, &arr, 2048, 0.01, 12).unwrap();
        assert!(
            0.000_001 > (-0.099_999_999_999_999_98 - tuning).abs(),
            "{tuning} !~= -0.09999999999999998"
        );
    }

    #[test]
    fn test_chroma_estimate_tuning_empty_fix() {
        assert!(0. == estimate_tuning(22050, &Array2::zeros((8192, 1)), 8192, 0.01, 12).unwrap());
    }

    #[test]
    fn test_estimate_tuning_decode() {
        let signal = Decoder::new()
            .unwrap()
            .decode(Path::new("data/s16_mono_22_5kHz.flac"))
            .unwrap()
            .samples;
        let stft = stft(&signal, 8192, 2205);

        let tuning = estimate_tuning(22050, &stft, 8192, 0.01, 12).unwrap();
        assert!(
            0.000_001 > (-0.049_999_999_999_999_99 - tuning).abs(),
            "{tuning} !~= -0.04999999999999999"
        );
    }

    #[test]
    fn test_pitch_tuning() {
        let file = File::open("data/pitch-tuning.npy").unwrap();
        let mut pitch = Array1::<f64>::read_npy(file).unwrap();
        let tuned = pitch_tuning(&mut pitch, 0.05, 12).unwrap();
        assert!(f64::EPSILON > (tuned + 0.1).abs(), "{tuned} != -0.1");
    }

    #[test]
    fn test_pitch_tuning_no_frequencies() {
        let mut frequencies = arr1(&[]);
        let tuned = pitch_tuning(&mut frequencies, 0.05, 12).unwrap();
        assert!(f64::EPSILON > tuned.abs(), "{tuned} != 0");
    }

    #[test]
    fn test_pip_track() {
        let file = File::open("data/spectrum-chroma.npy").unwrap();
        let spectrum = Array2::<f64>::read_npy(file).unwrap();

        let mags_file = File::open("data/spectrum-chroma-mags.npy").unwrap();
        let expected_mags = Array1::<f64>::read_npy(mags_file).unwrap();

        let pitches_file = File::open("data/spectrum-chroma-pitches.npy").unwrap();
        let expected_pitches = Array1::<f64>::read_npy(pitches_file).unwrap();

        let (mut pitches, mut mags) = pip_track(22050, &spectrum, 2048).unwrap();
        pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());
        mags.sort_by(|a, b| a.partial_cmp(b).unwrap());

        for (expected_pitches, actual_pitches) in expected_pitches.iter().zip(pitches.iter()) {
            assert!(
                0.000_000_01 > (expected_pitches - actual_pitches).abs(),
                "{expected_pitches} !~= {actual_pitches}"
            );
        }
        for (expected_mags, actual_mags) in expected_mags.iter().zip(mags.iter()) {
            assert!(
                0.000_000_01 > (expected_mags - actual_mags).abs(),
                "{expected_mags} !~= {actual_mags}"
            );
        }
    }

    #[test]
    fn test_chroma_filter() {
        let file = File::open("data/chroma-filter.npy").unwrap();
        let expected_filter = Array2::<f64>::read_npy(file).unwrap();

        let filter = chroma_filter(22050, 2048, 12, -0.1).unwrap();

        assert!(filter.iter().all(|&x| x > 0.));

        for (expected, actual) in expected_filter.iter().zip(filter.iter()) {
            assert!(
                0.000_000_001 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[rstest::rstest]
    // High 6 should be a major triad, 7 minor, 8 diminished and 9 augmented.
    #[case::major_triad("data/chroma/Cmaj.ogg", 6)]
    #[case::major_triad("data/chroma/Dmaj.ogg", 6)]
    #[case::minor_triad("data/chroma/Cmin.ogg", 7)]
    #[case::diminished_triad("data/chroma/Cdim.ogg", 8)]
    #[case::augmented_triad("data/chroma/Caug.ogg", 9)]
    fn test_end_result_triads(
        #[case] path: &str,
        #[case] expected_dominant_chroma_feature_index: usize,
    ) {
        let song = Decoder::new().unwrap().decode(Path::new(path)).unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let chroma_values = chroma_desc.get_value();

        let mut indices: Vec<usize> = (0..chroma_values.len()).collect();
        indices.sort_by(|&i, &j| chroma_values[j].partial_cmp(&chroma_values[i]).unwrap());
        assert!(indices[0] == expected_dominant_chroma_feature_index);
        for (i, v) in chroma_values.into_iter().enumerate() {
            if i >= 6 && i <= 10 {
                if i == expected_dominant_chroma_feature_index {
                    assert!(v > 0.8);
                } else {
                    assert!(v < 0.0);
                }
            }
        }
    }

    #[test]
    fn test_end_l2_norm_dyad() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/chroma/dyad_tritone_IC6.ogg"))
            .unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let chroma_values = chroma_desc.get_value();
        assert!(chroma_values[10] > 0.9);
    }

    #[test]
    fn test_end_l2_norm_mode() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/chroma/Cmaj_triads.ogg"))
            .unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let chroma_values = chroma_desc.get_value();
        assert!(chroma_values[11] > 0.9);
    }

    #[test]
    fn test_end_l2_norm_ratio() {
        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/chroma/triad_aug_maximize_ratio.ogg"))
            .unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let chroma_values = chroma_desc.get_value();
        assert!(chroma_values[12] > 0.7);
    }

    #[rstest::rstest]
    // Test all 12 intervals.
    #[case::minor_second("data/chroma/minor_second.ogg", 0)]
    #[case::major_second("data/chroma/major_second.ogg", 1)]
    #[case::minor_third("data/chroma/minor_third.ogg", 2)]
    #[case::major_third("data/chroma/major_third.ogg", 3)]
    #[case::perfect_fourth("data/chroma/perfect_fourth.ogg", 4)]
    #[case::tritone("data/chroma/tritone.ogg", 5)]
    #[case::perfect_fifth("data/chroma/perfect_fifth.ogg", 4)]
    #[case::minor_sixth("data/chroma/minor_sixth.ogg", 3)]
    #[case::major_sixth("data/chroma/major_sixth.ogg", 2)]
    #[case::minor_seventh("data/chroma/minor_seventh.ogg", 1)]
    #[case::major_seventh("data/chroma/major_seventh.ogg", 0)]
    fn test_end_result_intervals(
        #[case] path: &str,
        #[case] expected_dominant_chroma_feature_index: usize,
    ) {
        let song = Decoder::new().unwrap().decode(Path::new(path)).unwrap();
        let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
        chroma_desc.do_(&song.samples).unwrap();
        let chroma_values = chroma_desc.get_value();

        let mut indices: Vec<usize> = (0..chroma_values.len()).collect();
        indices.sort_by(|&i, &j| chroma_values[j].partial_cmp(&chroma_values[i]).unwrap());
        assert_eq!(indices[0], expected_dominant_chroma_feature_index);
        for (i, v) in chroma_values.into_iter().enumerate() {
            if i < 6 {
                if i == expected_dominant_chroma_feature_index {
                    assert!(v > 0.9);
                } else {
                    assert!(v < 0.0);
                }
            }
        }
    }
}
