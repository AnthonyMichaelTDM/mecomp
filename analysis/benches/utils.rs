use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mecomp_analysis::decoder::Decoder as DecoderTrait;
use mecomp_analysis::decoder::MecompDecoder as Decoder;
use mecomp_analysis::utils::{geometric_mean, reflect_pad, stft};
use ndarray::Array;
use std::path::Path;

fn bench_compute_stft(c: &mut Criterion) {
    let signal = Decoder::new()
        .unwrap()
        .decode(Path::new("data/piano.flac"))
        .unwrap()
        .samples;

    c.bench_function("mecomp-analysis: utils.rs: stft", |b| {
        b.iter(|| {
            let _ = stft(black_box(&signal), black_box(2048), black_box(512));
        });
    });
}

fn bench_reflect_pad(c: &mut Criterion) {
    let array = Array::range(0., 1_000_000., 1.);

    c.bench_function("mecomp-analysis: utils.rs: reflect_pad", |b| {
        b.iter(|| {
            let _ = reflect_pad(black_box(array.as_slice().unwrap()), black_box(3));
        });
    });
}

#[allow(clippy::too_many_lines)]
fn bench_geometric_mean(c: &mut Criterion) {
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
    c.bench_function("mecomp-analysis: utils.rs: geometric_mean", |b| {
        b.iter(|| {
            let _ = geometric_mean(black_box(&input));
        });
    });
}

criterion_group!(
    benches,
    bench_compute_stft,
    bench_reflect_pad,
    bench_geometric_mean
);
criterion_main!(benches);
