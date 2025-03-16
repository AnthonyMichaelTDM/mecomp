use log::warn;
use ndarray::{arr1, s, Array, Array1, Array2};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::f32::consts::PI;

use crate::Feature;

#[must_use]
#[inline]
pub fn reflect_pad(array: &[f32], pad: usize) -> Vec<f32> {
    debug_assert!(pad < array.len(), "Padding is too large");
    let prefix = array[1..=pad].iter().rev().copied().collect::<Vec<f32>>();
    let suffix = array[(array.len() - 2) - pad + 1..array.len() - 1]
        .iter()
        .rev()
        .copied()
        .collect::<Vec<f32>>();
    let mut output = Vec::with_capacity(prefix.len() + array.len() + suffix.len());

    output.extend(prefix);
    output.extend(array);
    output.extend(suffix);
    output
}

#[must_use]
#[allow(clippy::missing_inline_in_public_items)]
pub fn stft(signal: &[f32], window_length: usize, hop_length: usize) -> Array2<f64> {
    debug_assert!(window_length % 2 == 0, "Window length must be even");
    debug_assert!(window_length < signal.len(), "Signal is too short");
    debug_assert!(hop_length < window_length, "Hop length is too large");
    // Take advantage of raw-major order to have contiguous window for the
    // `assign`, reversing the axes to have the expected shape at the end only.
    let mut stft = Array2::zeros((signal.len().div_ceil(hop_length), window_length / 2 + 1));
    let signal = reflect_pad(signal, window_length / 2);

    // Periodic, so window_size + 1
    let mut hann_window = Array::zeros(window_length + 1);
    #[allow(clippy::cast_precision_loss)]
    for n in 0..window_length {
        hann_window[[n]] =
            0.5f32.mul_add(-f32::cos(2. * n as f32 * PI / (window_length as f32)), 0.5);
    }
    hann_window = hann_window.slice_move(s![0..window_length]);
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(window_length);

    for (window, mut stft_col) in signal
        .windows(window_length)
        .step_by(hop_length)
        .zip(stft.rows_mut())
    {
        let mut signal = (arr1(window) * &hann_window).mapv(|x| Complex::new(x, 0.));
        if let Some(s) = signal.as_slice_mut() {
            fft.process(s);
        } else {
            warn!("non-contiguous slice found for stft; expect slow performances.");
            fft.process(&mut signal.to_vec());
        }

        stft_col.assign(
            &signal
                .slice(s![..=window_length / 2])
                .mapv(|x| f64::from(x.re.hypot(x.im))),
        );
    }
    stft.permuted_axes((1, 0))
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn mean<T: Clone + Into<f32>>(input: &[T]) -> f32 {
    if input.is_empty() {
        return 0.;
    }
    input.iter().map(|x| x.clone().into()).sum::<f32>() / input.len() as f32
}

pub(crate) trait Normalize {
    const MAX_VALUE: Feature;
    const MIN_VALUE: Feature;

    fn normalize(&self, value: Feature) -> Feature {
        2. * (value - Self::MIN_VALUE) / (Self::MAX_VALUE - Self::MIN_VALUE) - 1.
    }
}

// Essentia algorithm
// https://github.com/MTG/essentia/blob/master/src/algorithms/temporal/zerocrossingrate.cpp
pub(crate) fn number_crossings(input: &[f32]) -> u32 {
    if input.is_empty() {
        return 0;
    }

    let mut crossings = 0;

    let mut was_positive = input[0] > 0.;

    for &sample in input {
        let is_positive = sample > 0.;
        if was_positive != is_positive {
            crossings += 1;
            was_positive = is_positive;
        }
    }

    crossings
}

/// Only works for input of size 256 (or at least of size a multiple
/// of 8), with values belonging to [0; 2^65].
///
/// This finely optimized geometric mean courtesy of
/// Jacques-Henri Jourdan (<https://jhjourdan.mketjh.fr/>)
#[must_use]
#[allow(clippy::missing_inline_in_public_items)]
pub fn geometric_mean(input: &[f32]) -> f32 {
    debug_assert_eq!(input.len() % 8, 0, "Input size must be a multiple of 8");
    if input.is_empty() {
        return 0.;
    }

    let mut exponents: i32 = 0;
    let mut mantissas: f64 = 1.;
    for ch in input.chunks_exact(8) {
        let mut m = (f64::from(ch[0]) * f64::from(ch[1])) * (f64::from(ch[2]) * f64::from(ch[3]));
        m *= 3.273_390_607_896_142e150; // 2^500 : avoid underflows and denormals
        m *= (f64::from(ch[4]) * f64::from(ch[5])) * (f64::from(ch[6]) * f64::from(ch[7]));
        if m == 0. {
            return 0.;
        }
        exponents += (m.to_bits() >> 52) as i32;
        mantissas *= f64::from_bits((m.to_bits() & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000);
    }

    #[allow(clippy::cast_possible_truncation)]
    let n = input.len() as u32;
    #[allow(clippy::cast_possible_truncation)]
    let result = (((mantissas.log2() + f64::from(exponents)) / f64::from(n) - (1023. + 500.) / 8.)
        .exp2()) as f32;
    result
}

pub(crate) fn hz_to_octs_inplace(
    frequencies: &mut Array1<f64>,
    tuning: f64,
    bins_per_octave: u32,
) -> &mut Array1<f64> {
    let a440 = 440.0 * (tuning / f64::from(bins_per_octave)).exp2();

    *frequencies /= a440 / 16.;
    frequencies.mapv_inplace(f64::log2);
    frequencies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder};
    use ndarray::{arr1, Array, Array2};
    use ndarray_npy::ReadNpyExt;
    use std::{fs::File, path::Path};

    #[test]
    fn test_mean() {
        let numbers = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let mean = mean(&numbers);
        assert!(f32::EPSILON > (2.0 - mean).abs(), "{mean} !~= 2.0");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_geometric_mean() {
        let numbers = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let mean = geometric_mean(&numbers);
        assert!(f32::EPSILON > (0.0 - mean).abs(), "{mean} !~= 0.0");

        let numbers = vec![4.0, 2.0, 1.0, 4.0, 2.0, 1.0, 2.0, 2.0];
        let mean = geometric_mean(&numbers);
        assert!(0.0001 > (2.0 - mean).abs(), "{mean} !~= 2.0");

        // never going to happen, but just in case
        let numbers = vec![256., 4.0, 2.0, 1.0, 4.0, 2.0, 1.0, 2.0];
        let mean = geometric_mean(&numbers);
        assert!(
            0.0001 > (3.668_016_2 - mean).abs(),
            "{mean} !~= {}",
            3.668_016_172_818_685
        );

        let subnormal = vec![4.0, 2.0, 1.0, 4.0, 2.0, 1.0, 2.0, 1.0e-40_f32];
        let mean = geometric_mean(&subnormal);
        assert!(
            0.0001 > (1.834_008e-5 - mean).abs(),
            "{} !~= {}",
            mean,
            1.834_008_086_409_341_7e-5
        );

        let maximum = vec![2_f32.powi(65); 256];
        let mean = geometric_mean(&maximum);
        assert!(
            0.0001 > (2_f32.powi(65) - mean.abs()),
            "{} !~= {}",
            mean,
            2_f32.powi(65)
        );

        let input = [
            0.024_454_033,
            0.088_096_89,
            0.445_543_62,
            0.827_535_03,
            0.158_220_93,
            1.444_224_5,
            3.697_138_5,
            3.678_955_6,
            1.598_157_2,
            1.017_271_8,
            1.443_609_6,
            3.145_710_2,
            2.764_110_8,
            0.839_523_5,
            0.248_968_29,
            0.070_631_73,
            0.355_419_4,
            0.352_001_4,
            0.797_365_1,
            0.661_970_8,
            0.784_104,
            0.876_795_7,
            0.287_382_66,
            0.048_841_28,
            0.322_706_5,
            0.334_907_47,
            0.185_888_75,
            0.135_449_42,
            0.140_177_46,
            0.111_815_82,
            0.152_631_61,
            0.221_993_12,
            0.056_798_387,
            0.083_892_57,
            0.070_009_65,
            0.202_903_29,
            0.370_717_38,
            0.231_543_18,
            0.023_348_59,
            0.013_220_183,
            0.035_887_096,
            0.029_505_49,
            0.090_338_57,
            0.176_795_04,
            0.081_421_87,
            0.003_326_808_6,
            0.012_269_007,
            0.016_257_336,
            0.027_027_424,
            0.017_253_408,
            0.017_230_038,
            0.021_678_915,
            0.018_645_158,
            0.005_417_136,
            0.006_650_174_5,
            0.020_159_671,
            0.026_623_515,
            0.005_166_793_7,
            0.016_880_387,
            0.009_935_223_5,
            0.011_079_361,
            0.013_200_151,
            0.005_320_572_3,
            0.005_070_289_6,
            0.008_130_498,
            0.009_006_041,
            0.003_602_499_8,
            0.006_440_387_6,
            0.004_656_151,
            0.002_513_185_8,
            0.003_084_559_7,
            0.008_722_531,
            0.017_871_628,
            0.022_656_294,
            0.017_539_924,
            0.009_439_588_5,
            0.003_085_72,
            0.001_358_616_6,
            0.002_746_787_2,
            0.005_413_010_3,
            0.004_140_312,
            0.000_143_587_14,
            0.001_371_840_8,
            0.004_472_961,
            0.003_769_122,
            0.003_259_129_6,
            0.003_637_24,
            0.002_445_332_2,
            0.000_590_368_93,
            0.000_647_898_65,
            0.001_745_297,
            0.000_867_165_5,
            0.002_156_236_2,
            0.001_075_606_8,
            0.002_009_199_5,
            0.001_537_388_5,
            0.000_984_620_4,
            0.000_292_002_49,
            0.000_921_162_4,
            0.000_535_111_8,
            0.001_491_276_5,
            0.002_065_137_5,
            0.000_661_122_26,
            0.000_850_054_26,
            0.001_900_590_1,
            0.000_639_584_5,
            0.002_262_803,
            0.003_094_018_2,
            0.002_089_161_7,
            0.001_215_059,
            0.001_311_408_4,
            0.000_470_959,
            0.000_665_480_7,
            0.001_430_32,
            0.001_791_889_3,
            0.000_863_200_75,
            0.000_560_445_5,
            0.000_828_417_54,
            0.000_669_453_9,
            0.000_822_765,
            0.000_616_575_8,
            0.001_189_319,
            0.000_730_024_5,
            0.000_623_748_1,
            0.001_207_644_4,
            0.001_474_674_2,
            0.002_033_916,
            0.001_500_169_9,
            0.000_520_51,
            0.000_445_643_32,
            0.000_558_462_75,
            0.000_897_786_64,
            0.000_805_247_05,
            0.000_726_536_44,
            0.000_673_052_6,
            0.000_994_064_5,
            0.001_109_393_7,
            0.001_295_099_7,
            0.000_982_682_2,
            0.000_876_651_8,
            0.001_654_928_7,
            0.000_929_064_35,
            0.000_291_306_23,
            0.000_250_490_47,
            0.000_228_488_02,
            0.000_269_673_15,
            0.000_237_375_09,
            0.000_969_406_1,
            0.001_063_811_8,
            0.000_793_428_86,
            0.000_590_835_06,
            0.000_476_389_9,
            0.000_951_664_1,
            0.000_692_231_46,
            0.000_557_113_7,
            0.000_851_769_7,
            0.001_071_027_7,
            0.000_610_243_9,
            0.000_746_876_23,
            0.000_849_898_44,
            0.000_495_806_2,
            0.000_526_994,
            0.000_215_249_22,
            0.000_096_684_314,
            0.000_654_554_4,
            0.001_220_697_3,
            0.001_210_358_3,
            0.000_920_454_33,
            0.000_924_843_5,
            0.000_812_128_4,
            0.000_239_532_56,
            0.000_931_822_4,
            0.001_043_966_3,
            0.000_483_734_15,
            0.000_298_952_22,
            0.000_484_425_4,
            0.000_666_829_5,
            0.000_998_398_5,
            0.000_860_489_7,
            0.000_183_153_23,
            0.000_309_180_8,
            0.000_542_646_2,
            0.001_040_391_5,
            0.000_755_456_6,
            0.000_284_601_7,
            0.000_600_979_3,
            0.000_765_056_9,
            0.000_562_810_46,
            0.000_346_616_55,
            0.000_236_224_32,
            0.000_598_710_6,
            0.000_295_684_27,
            0.000_386_978_06,
            0.000_584_258,
            0.000_567_097_6,
            0.000_613_644_4,
            0.000_564_549_3,
            0.000_235_384_52,
            0.000_285_574_6,
            0.000_385_352_93,
            0.000_431_935_65,
            0.000_731_246_5,
            0.000_603_072_8,
            0.001_033_130_8,
            0.001_195_216_2,
            0.000_824_500_7,
            0.000_422_183_63,
            0.000_821_760_16,
            0.001_132_246,
            0.000_891_406_73,
            0.000_635_158_8,
            0.000_372_681_56,
            0.000_230_35,
            0.000_628_649_3,
            0.000_806_159_9,
            0.000_661_622_15,
            0.000_227_139_01,
            0.000_214_694_96,
            0.000_665_457_7,
            0.000_513_901,
            0.000_391_766_78,
            0.001_079_094_7,
            0.000_735_363_7,
            0.000_171_665_73,
            0.000_439_648_87,
            0.000_295_145_3,
            0.000_177_047_08,
            0.000_182_958_97,
            0.000_926_536_04,
            0.000_832_408_3,
            0.000_804_168_4,
            0.001_131_809_3,
            0.001_187_149_6,
            0.000_806_948_8,
            0.000_628_624_75,
            0.000_591_386_1,
            0.000_472_182_3,
            0.000_163_652_31,
            0.000_177_876_57,
            0.000_425_363_75,
            0.000_573_699_3,
            0.000_434_679_24,
            0.000_090_282_94,
            0.000_172_573_55,
            0.000_501_957_4,
            0.000_614_716_8,
            0.000_216_780_5,
            0.000_148_974_3,
            0.000_055_081_473,
            0.000_296_264_13,
            0.000_378_055_67,
            0.000_147_361_96,
            0.000_262_513_64,
            0.000_162_118_42,
            0.000_185_347_7,
            0.000_138_735_4,
        ];
        assert!(
            0.000_000_01 > (0.002_575_059_7 - geometric_mean(&input)).abs(),
            "{} !~= 0.0025750597",
            geometric_mean(&input)
        );
    }

    #[test]
    fn test_hz_to_octs_inplace() {
        let mut frequencies = arr1(&[32., 64., 128., 256.]);
        let expected = arr1(&[0.168_640_29, 1.168_640_29, 2.168_640_29, 3.168_640_29]);

        hz_to_octs_inplace(&mut frequencies, 0.5, 10)
            .iter()
            .zip(expected.iter())
            .for_each(|(x, y)| assert!(0.0001 > (x - y).abs(), "{x} !~= {y}"));
    }

    #[test]
    fn test_compute_stft() {
        let file = File::open("data/librosa-stft.npy").unwrap();
        let expected_stft = Array2::<f32>::read_npy(file).unwrap().mapv(f64::from);

        let song = Decoder::new()
            .unwrap()
            .decode(Path::new("data/piano.flac"))
            .unwrap();

        let stft = stft(&song.samples, 2048, 512);

        assert!(!stft.is_empty() && !expected_stft.is_empty(), "Empty STFT");
        for (expected, actual) in expected_stft.iter().zip(stft.iter()) {
            // NOTE: can't use relative error here due to division by zero
            assert!(
                0.0001 > (expected - actual).abs(),
                "{expected} !~= {actual}"
            );
        }
    }

    #[test]
    fn test_reflect_pad() {
        let array = Array::range(0., 100_000., 1.);

        let output = reflect_pad(array.as_slice().unwrap(), 3);
        assert_eq!(&output[..4], &[3.0, 2.0, 1.0, 0.]);
        assert_eq!(&output[3..100_003], array.to_vec());
        assert_eq!(&output[100_003..100_006], &[99998.0, 99997.0, 99996.0]);
    }
}
