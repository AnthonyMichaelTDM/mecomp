//! Implementation of a few reusable kernels accelerated with SIMD instructions when available
//!
//! ## Kernels
//!
//! ### multiply
//!
//! Element-wise multiplication of two arrays of the same length
//!
//! `a[i] = a[i] * b[i]`
//!
//! ### dot
//!
//! dot product of two arrays of the same length
//!
//! `result = sum(a[i] * b[i])`
//!
//! ### Sum and index weighted sum
//!
//! computes the sum and the index weighted sum of an array
//!
//! `result = sum(a[i])`
//! `weighted_result = sum(i * a[i])`
//!
//! ### column-wise scaled sum
//!
//! Computes the sum over each column (plane) scaled by a factor
//!
//! `out[i] = sum_j(planes[j][i] * scale)`
//!
//! ### Lagged cross-correlation
//!
//! Computes the cross-autocorrelation of an array with itself for different lags
//!
//! `output[i] = sum_{j=i..n}(a[j - i] * b[j]) / (n - i)`

use std::ops::AddAssign;

use fearless_simd::{Level, dispatch, prelude::*};
use fearless_simd_macros::simd;

#[simd]
fn multiply_f32s<S: Simd>(simd: S, a: &mut [f32], b: &[f32]) {
    let len = a.len().min(b.len());

    let mut a_chunks = a[..len].chunks_exact_mut(S::f32s::LEN); // the CPU's native SIMD width
    let mut b_chunks = b[..len].chunks_exact(S::f32s::LEN);

    for (a_chunk, b_chunk) in (&mut a_chunks).zip(b_chunks.by_ref()) {
        let a_simd = S::f32s::from_slice(simd, a_chunk);
        let b_simd = S::f32s::from_slice(simd, b_chunk);
        (a_simd * b_simd).store_slice(a_chunk);
    }

    for (a_rem, b_rem) in a_chunks
        .into_remainder()
        .iter_mut()
        .zip(b_chunks.remainder())
    {
        *a_rem *= *b_rem;
    }
}

/// Element-wise multiplication of two arrays of the same length
///
/// `a[i] = a[i] * b[i]`
pub fn multiply(a: &mut [f32], b: &[f32]) {
    let level = Level::new();
    dispatch!(level, simd => multiply_f32s(simd, a, b));
}

#[simd]
fn dot_f32s<S: Simd>(simd: S, a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let mut sum = S::f32s::splat(simd, 0.0);

    let mut a_chunks = a[..len].chunks_exact(S::f32s::LEN);
    let mut b_chunks = b[..len].chunks_exact(S::f32s::LEN);

    for (a_chunk, b_chunk) in (&mut a_chunks).zip(b_chunks.by_ref()) {
        let a_simd = S::f32s::from_slice(simd, a_chunk);
        let b_simd = S::f32s::from_slice(simd, b_chunk);
        sum = a_simd.mul_add(b_simd, sum);
    }

    let mut result = sum.reduce_sum();

    // process remainder
    for (a_rem, b_rem) in a_chunks.remainder().iter().zip(b_chunks.remainder()) {
        result += a_rem * b_rem;
    }

    result
}

/// Dot product of two arrays of the same length
///
/// `result = sum(a[i] * b[i])`
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    let level = Level::new();
    dispatch!(level, simd => dot_f32s(simd, a, b))
}

#[simd]
fn sum_and_index_weighted_sum_f32s<S: Simd>(simd: S, a: &[f32]) -> (f32, f32) {
    let mut sum = S::f32s::splat(simd, 0.0);
    let mut weighted_sum = S::f32s::splat(simd, 0.0);

    #[allow(clippy::cast_precision_loss)]
    let lane_indices = S::f32s::from_fn(
        simd,
        #[inline(always)]
        |i| i as f32,
    );

    let mut a_chunks = a.chunks_exact(S::f32s::LEN);
    let num_chunks = a_chunks.len();

    for (block_index, a_chunk) in a_chunks.by_ref().enumerate() {
        let a_simd = S::f32s::from_slice(simd, a_chunk);
        #[allow(clippy::cast_precision_loss)]
        let indices = lane_indices + S::f32s::splat(simd, (block_index * S::f32s::LEN) as f32);
        sum.add_assign(a_simd);
        weighted_sum = a_simd.mul_add(indices, weighted_sum);
    }

    let mut result = sum.reduce_sum();
    let mut weighted_result = weighted_sum.reduce_sum();

    #[allow(clippy::cast_precision_loss)]
    for (i, &value) in a_chunks.remainder().iter().enumerate() {
        result += value;
        weighted_result += (i + num_chunks * S::f32s::LEN) as f32 * value;
    }

    (result, weighted_result)
}

/// Sum and index weighted sum of an array
///
/// `result = sum(a[i])`
/// `weighted_result = sum(i * a[i])`
pub fn sum_and_index_weighted_sum(a: &[f32]) -> (f32, f32) {
    let level = Level::new();
    dispatch!(level, simd => sum_and_index_weighted_sum_f32s(simd, a))
}

#[simd]
fn column_wise_scaled_sum_f32s<S: Simd>(
    simd: S,
    planes: &[&[f32]],
    output: &mut [f32],
    scale: f32,
) {
    let shortest_plane = planes.iter().map(|p| p.len()).min().unwrap_or_default();
    let len = shortest_plane.min(output.len());

    // SIMD implementation goes here
    for plane in planes {
        let mut output_chunks = output[..len].chunks_exact_mut(S::f32s::LEN);

        let mut plane_chunks = plane[..len].chunks_exact(S::f32s::LEN);
        let plane_remainder = plane_chunks.remainder();

        for (output_chunk, plane_chunk) in output_chunks.by_ref().zip(plane_chunks.by_ref()) {
            let output_simd = S::f32s::from_slice(simd, output_chunk);
            let plane_simd = S::f32s::from_slice(simd, plane_chunk);
            plane_simd
                .mul_add(scale, output_simd)
                .store_slice(output_chunk);
        }

        for (output_sample, &plane_sample) in output_chunks
            .into_remainder()
            .iter_mut()
            .zip(plane_remainder)
        {
            *output_sample += plane_sample * scale;
        }
    }
}

/// Column-wise scaled sum of multiple planes
///
/// Each element of the output array is the sum of the corresponding elements of the input planes, scaled by `scale`.
///
/// `out[i] += sum_j(planes[j][i] * scale)`
pub fn column_wise_scaled_sum(planes: &[&[f32]], output: &mut [f32], scale: f32) {
    // output.fill(0.0);

    let level = Level::new();
    dispatch!(level, simd => column_wise_scaled_sum_f32s(simd, planes, output, scale));
}

#[simd]
fn lagged_cross_correlation_f32s<S: Simd>(simd: S, a: &[f32], b: &[f32], output: &mut [f32]) {
    let n = a.len().min(output.len());

    #[allow(clippy::cast_precision_loss)]
    let indices = S::f32s::from_fn(
        simd,
        #[inline(always)]
        |i| i as f32,
    );
    let mut output_chunks = output[..n].chunks_exact_mut(S::f32s::LEN);
    let n_output_chunks = output_chunks.len();

    // Blocks of LANES lags: lane k handles lag `block_start + k`.
    for (block_index, output_chunk) in output_chunks.by_ref().enumerate() {
        let block_start = block_index * S::f32s::LEN;
        let mut acc = S::f32s::splat(simd, 0.0);

        // Lane k starts at block_start + k, so only the leading lanes contribute in this
        // prologue.
        for j in 0..S::f32s::LEN - 1 {
            let x = b[block_start + j]; // right-hand sample
            for k in 0..=j {
                acc[k] += a[j - k] * x;
            }
        }

        // All lanes are active; each lane k handles lag block_start + k.
        for (window_start, window) in a[..n - block_start].windows(S::f32s::LEN).enumerate() {
            // x = the right-hand sample corresponding to the end of the current window
            let x = b[window_start + block_start + S::f32s::LEN - 1]; // right-hand sample
            let window_simd = S::f32s::from_slice(simd, window);
            let reversed = window_simd.reverse();
            acc = reversed.mul_add(x, acc);
        }

        #[allow(clippy::cast_precision_loss)]
        let divisor = S::f32s::splat(simd, (n - block_start) as f32) - indices;
        (acc / divisor).store_slice(output_chunk);
    }

    // remaining lags (fewer than a full SIMD block)
    #[allow(clippy::cast_precision_loss)]
    for (offset, output_sample) in output_chunks.into_remainder().iter_mut().enumerate() {
        let lag = n_output_chunks * S::f32s::LEN + offset;
        let mut sum = 0.0;
        for j in lag..n {
            sum += a[j - lag] * b[j];
        }
        *output_sample = sum / (n - lag) as f32;
    }
}

/// Lagged cross-correlation
///
/// Computes the cross-correlation between two signals for different lags.
///
/// output[i] += sum_{j=i..n}(a[j - i] * b[j]) / (n - i)
pub fn lagged_cross_correlation(a: &[f32], b: &[f32], output: &mut [f32]) {
    // output.fill(0.0);

    let level = Level::new();
    dispatch!(level, simd => lagged_cross_correlation_f32s(simd, a, b, output));

    // let n = a.len();
    // for i in 0..output.len() {
    //     let mut sum = 0.0;
    //     for j in i..n {
    //         sum += a[j - i] * a[j];
    //     }
    //     output[i] = sum / (n - i) as f32;
    // }
}

// #[cfg(target_arch = "x86_64")]
// #[target_feature(enable = "avx2")]
// unsafe fn lagged_dot_products_avx2<const FMA: bool>(
//     left: &[f32],
//     right: &[f32],
//     output: &mut [f32],
// ) {
//     use std::arch::x86_64::{
//         _mm256_add_ps, _mm256_broadcast_ss, _mm256_loadu_ps, _mm256_mul_ps,
//         _mm256_permutevar8x32_ps, _mm256_setr_epi32, _mm256_storeu_ps,
//     };

//     let n = left.len();
//     let reverse = _mm256_setr_epi32(7, 6, 5, 4, 3, 2, 1, 0);

//     let (output_chunks, output_remainder) = output.as_chunks_mut::<WIDTH>();
//     for (block_index, output_chunk) in output_chunks.iter_mut().enumerate() {
//         let block_start = block_index * WIDTH;
//         let mut initial = [0.0; WIDTH];
//         for j in block_start..block_start + WIDTH - 1 {
//             let x = right[j];
//             for k in 0..=(j - block_start) {
//                 initial[k] = madd_f32::<FMA>(left[j - block_start - k], x, initial[k]);
//             }
//         }

//         // SAFETY: `initial` contains exactly WIDTH floats.
//         let mut acc = unsafe { _mm256_loadu_ps(initial.as_ptr()) };
//         for (j, &right_value) in right.iter().enumerate().skip(block_start + WIDTH - 1) {
//             let x = _mm256_broadcast_ss(&right_value);
//             let window_start = j - block_start + 1 - WIDTH;
//             // SAFETY: each window spans 8 input values ending at j - block_start.
//             let window = unsafe { _mm256_loadu_ps(left.as_ptr().add(window_start)) };
//             let reversed = _mm256_permutevar8x32_ps(window, reverse);
//             acc = if FMA {
//                 // SAFETY: the caller selects this specialization only when FMA is available.
//                 unsafe { std::arch::x86_64::_mm256_fmadd_ps(reversed, x, acc) }
//             } else {
//                 let products = _mm256_mul_ps(reversed, x);
//                 _mm256_add_ps(acc, products)
//             };
//         }

//         // SAFETY: output_chunk contains exactly WIDTH writable floats.
//         unsafe {
//             _mm256_storeu_ps(output_chunk.as_mut_ptr(), acc);
//         }
//     }

//     #[allow(clippy::cast_precision_loss)]
//     for (offset, output) in output_remainder.iter_mut().enumerate() {
//         let lag = output_chunks.len() * WIDTH + offset;
//         let mut sum = 0.0;
//         for j in lag..n {
//             sum = madd_f32::<FMA>(left[j - lag], right[j], sum);
//         }
//         *output = sum;
//     }
// }

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::basic(&mut [1.0; 10], &[1.0; 10], &[1.0; 10])]
    #[case::basic(&mut [2.0; 10], &[2.0; 10], &[4.0; 10])]
    #[case::longer_data(&mut [1.0; 10], &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 1.0])]
    #[case::longer_weights(&mut [1.0; 5], &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0], &[2.0, 3.0, 4.0, 5.0, 6.0])]
    fn test_multiply(
        #[case] mut data: &mut [f32],
        #[case] weights: &[f32],
        #[case] expected: &[f32],
    ) {
        let mut data_copy = data.to_vec();
        multiply(&mut data, &weights);
        assert_eq!(data, expected);

        let level = Level::baseline();
        dispatch!(level, simd => multiply_f32s(simd, &mut data_copy, &weights));
        assert_eq!(data_copy, expected);
    }

    #[rstest]
    #[case::basic(&[1.0; 10], &[1.0; 10], 10.0)]
    #[case::zeros(&[0.0; 10], &[0.0; 10], 0.0)]
    #[case::mixed(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], 32.0)]
    #[case::longer(&[1.0; 15], &[2.0; 10], 20.0)]
    #[case::shorter(&[1.0; 5], &[2.0; 10], 10.0)]
    #[case::chunks_and_remainders(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0], 285.0)]
    fn test_dot(#[case] left: &[f32], #[case] right: &[f32], #[case] expected: f32) {
        assert_eq!(dot(left, right), expected);

        let level = Level::baseline();
        assert_eq!(
            dispatch!(level, simd => dot_f32s(simd, &left, &right)),
            expected,
        );
    }

    #[rstest]
    #[case::basic(&[1.0, 2.0, 3.0], (6.0, 8.0))]
    #[case::zeros(&[0.0; 3], (0.0, 0.0))]
    #[case::long(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0], (45.0, 240.0))]
    #[case::longer(&[1.5; 33], (49.5, 792.))]
    fn test_sum_and_index_weighted_sum(#[case] data: &[f32], #[case] expected: (f32, f32)) {
        assert_eq!(sum_and_index_weighted_sum(data), expected);
        let level = Level::baseline();
        assert_eq!(
            dispatch!(level, simd => sum_and_index_weighted_sum_f32s(simd, &data)),
            expected
        );
    }

    #[rstest]
    #[case::basic(&[&[1.0, 2.0, 3.0], [1.0, 2.0, 3.0].as_slice()], 2.0, &[4.0, 8.0, 12.0])]
    #[case::empty(&[], 0.5, &[0.0; 9])]
    #[case::mismatched(&[[1.0, 2.0].as_slice(), [0.5; 15].as_slice()], 1.0, &[1.5, 2.5, 0.0,0.0,])]
    #[case::long(&[[1.0; 10].as_slice(), [2.0; 10].as_slice()], 1.0, &[3.0; 10])]
    #[case::longer(&[[1.0; 33].as_slice(), [2.0; 33].as_slice()], 1.0, &[3.0; 33])]
    #[case::chunks_and_remainder(&[[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0].as_slice(), &[0.5; 10]], 0.5, &[0.75, 1.25, 1.75, 2.25, 2.75, 3.25, 3.75, 4.25, 4.75, 5.25])]
    fn test_column_wise_scaled_sum(
        #[case] planes: &[&[f32]],
        #[case] scale: f32,
        #[case] expected: &[f32],
    ) {
        let mut output = expected.to_vec();
        output.fill(0.0);
        let mut output_copy = output.clone();

        column_wise_scaled_sum(planes, &mut output_copy, scale);
        assert_eq!(output_copy, expected);

        let level = Level::baseline();
        dispatch!(level, simd => column_wise_scaled_sum_f32s(simd, planes, &mut output, scale));
        assert_eq!(output, expected);
    }

    #[test]
    fn column_wise_scaled_sum_with_nonzero_initial_output() {
        let mut output = [PI; 9];

        column_wise_scaled_sum(&[], &mut output, 2.0);

        assert_eq!(output, [PI; 9]); // unchanged
    }

    #[rstest]
    #[case::basic(&[1.0, 2.0, 3.0], &[1.0, 1.0, 1.5], &[2.5, 2.0, 1.5])]
    #[case::empty(&[], &[0.], &[])]
    #[case::empty(&[0.], &[], &[])]
    #[case::empty(&[], &[], &[])]
    #[case::mismatched(&[1.0, 2.0], &[0.5; 15], &[])]
    #[case::mismatched(&[1.0, 2.0], &[0.5; 15], &[0.75,0.5,0.,0.,])]
    #[case::long(&[1.0; 10], &[0.5; 10], &[0.5; 10])]
    #[case::longer(&[1.0; 33], &[0.5; 33], &[0.5; 33])]
    fn test_lagged_cross_correlation(
        #[case] left: &[f32],
        #[case] right: &[f32],
        #[case] expected: &[f32],
    ) {
        let mut output = vec![0.0; expected.len()];
        lagged_cross_correlation(left, right, &mut output);
        assert_eq!(output, expected);

        let mut output = vec![0.0; expected.len()];
        let level = Level::baseline();
        dispatch!(level, simd => lagged_cross_correlation_f32s(simd, left, right, &mut output));
        assert_eq!(output, expected);
    }

    #[test]
    fn lagged_cross_correlation_match_reference_for_distinct_inputs() {
        let left = [0.5, -0.25, 0.75, 0.1, -0.4, 0.6, 0.3, -0.2, 0.8, 0.1];
        let right = [-0.4, 0.6, 0.3, -0.2, 0.8, 0.1, 0.5, -0.25, 0.75, 0.1];

        // run on best available SIMD level
        let mut output = [0.0; 10];
        lagged_cross_correlation(&left, &right, &mut output);
        // run on baseline
        let mut baseline_output = [0.0; 10];
        dispatch!(Level::baseline(), simd => lagged_cross_correlation_f32s(simd, &left, &right, &mut baseline_output));

        for (lag, (&best_actual, &baseline_actual)) in
            output.iter().zip(baseline_output.iter()).enumerate()
        {
            let expected = (lag..left.len())
                .map(|index| left[index - lag] * right[index] / (10. - lag as f32))
                .sum::<f32>();
            assert!((best_actual - expected).abs() < 1e-6);
            assert!((baseline_actual - expected).abs() < 1e-6);
            assert!((best_actual - baseline_actual).abs() < 1e-6);
            assert_eq!(best_actual, baseline_actual);
        }
    }

    #[test]
    fn lagged_cross_correlation_matches_reference_for_delayed_tones() {
        const N: usize = 37;
        const DELAY: usize = 3;

        let left: Vec<f32> = (0..N)
            .map(|i| {
                let t = i as f32;
                0.7 * (2.0 * PI * t / 13.0).sin() + 0.25 * (2.0 * PI * t / 5.0).sin()
            })
            .collect();
        let right: Vec<f32> = (0..N)
            .map(|i| {
                let delayed = i.checked_sub(DELAY).map_or(0.0, |j| left[j]);
                let noise = 0.02 * (2.0 * PI * i as f32 / 7.0).sin();
                delayed + noise
            })
            .collect();

        let mut dispatched = [0.0; N];
        lagged_cross_correlation(&left, &right, &mut dispatched);

        let mut baseline = [0.0; N];
        dispatch!(
            Level::baseline(),
            simd => lagged_cross_correlation_f32s(simd, &left, &right, &mut baseline)
        );

        for lag in 0..N {
            let sum = (lag..N).map(|j| left[j - lag] * right[j]).sum::<f32>();
            let expected = sum / (N - lag) as f32;

            assert!(
                (dispatched[lag] - expected).abs() <= 1e-6,
                "dispatched lag {lag}: {} != {expected}",
                dispatched[lag]
            );
            assert!(
                (baseline[lag] - expected).abs() <= 1e-6,
                "baseline lag {lag}: {} != {expected}",
                baseline[lag]
            );
        }
    }
}
