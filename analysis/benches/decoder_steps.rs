//! Benchmarks for each step of the decoding process.
use std::{fs::File, hint::black_box, path::Path};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mecomp_analysis::decoder::{
    Decoder as DecoderTrait, MecompDecoder as Decoder, SymphoniaSource,
};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};

fn bench_decoder_steps_symphonia_source(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .canonicalize()
        .unwrap();

    let mut group = c.benchmark_group("mecomp-analysis: decoder.rs: SymphoniaSource");

    let files = &[
        "s16_mono_22_5kHz.flac",
        "s16_stereo_22_5kHz.flac",
        "s32_mono_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.mp3",
        "5_mins_of_noise_stereo_48kHz.ogg",
    ];

    for file in files {
        group.bench_with_input(
            BenchmarkId::from_parameter(file),
            &path.join(file),
            |b, path| {
                b.iter_with_setup(
                    || {
                        let file = Box::new(File::open(path).unwrap());
                        MediaSourceStream::new(file, MediaSourceStreamOptions::default())
                    },
                    |mss| {
                        let source = SymphoniaSource::new(mss).unwrap();
                        black_box(source.into_iter().collect::<Vec<f32>>());
                    },
                );
            },
        );
    }
}

fn bench_decoder_steps_downmix(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .canonicalize()
        .unwrap();

    let mut group =
        c.benchmark_group("mecomp-analysis: decoder.rs: MecompDecoder::into_mono_samples");

    let files = &[
        "s16_mono_22_5kHz.flac",
        "s16_stereo_22_5kHz.flac",
        "s32_mono_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.mp3",
        "5_mins_of_noise_stereo_48kHz.ogg",
    ];

    for file in files {
        group.bench_with_input(
            BenchmarkId::from_parameter(file),
            &path.join(file),
            |b, path| {
                let source = SymphoniaSource::new(MediaSourceStream::new(
                    Box::new(File::open(path).unwrap()),
                    MediaSourceStreamOptions::default(),
                ))
                .unwrap();
                let channels = source.channels();
                let samples: Vec<f32> = source.collect();
                b.iter_with_setup(
                    || (samples.clone(), channels),
                    |(samples, channels)| {
                        let mono_samples = Decoder::into_mono_samples(samples, channels).unwrap();

                        black_box(mono_samples);
                    },
                );
            },
        );
    }
}

fn bench_decoder_steps_resample(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .canonicalize()
        .unwrap();

    let mut group =
        c.benchmark_group("mecomp-analysis: decoder.rs: MecompDecoder::resample_mono_samples");

    let files = &[
        "s16_mono_22_5kHz.flac",
        "s16_stereo_22_5kHz.flac",
        "s32_mono_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.mp3",
        "5_mins_of_noise_stereo_48kHz.ogg",
    ];

    for file in files {
        group.bench_with_input(
            BenchmarkId::from_parameter(file),
            &path.join(file),
            |b, path| {
                let source = SymphoniaSource::new(MediaSourceStream::new(
                    Box::new(File::open(path).unwrap()),
                    MediaSourceStreamOptions::default(),
                ))
                .unwrap();
                let sample_rate = source.sample_rate();
                let channels = source.channels();
                let samples: Vec<f32> =
                    Decoder::into_mono_samples(source.collect(), channels).unwrap();
                b.iter_with_setup(
                    || (samples.clone(), sample_rate, Decoder::new().unwrap()),
                    |(samples, sample_rate, decoder)| {
                        let resampled_samples =
                            decoder.resample_mono_samples(samples, sample_rate).unwrap();

                        black_box(resampled_samples);
                    },
                );
            },
        );
    }
}

fn bench_decoder_end2end(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .canonicalize()
        .unwrap();

    let mut group = c.benchmark_group("mecomp-analysis: decoder.rs: MecompDecoder::decode");

    let files = &[
        "s16_mono_22_5kHz.flac",
        "s16_stereo_22_5kHz.flac",
        "s32_mono_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.flac",
        "s32_stereo_44_1_kHz.mp3",
        "5_mins_of_noise_stereo_48kHz.ogg",
    ];

    for file in files {
        group.bench_with_input(
            BenchmarkId::from_parameter(file),
            &path.join(file),
            |b, path| {
                b.iter_with_setup(
                    || Decoder::new().unwrap(),
                    |decoder| {
                        decoder.decode(black_box(path)).unwrap();
                    },
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_decoder_steps_symphonia_source,
    bench_decoder_steps_downmix,
    bench_decoder_steps_resample,
);
criterion_group!(
    name = e2e;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(20));
    targets = bench_decoder_end2end
);
criterion_main!(benches, e2e);
