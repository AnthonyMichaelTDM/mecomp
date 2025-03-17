use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::{
    decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder, SymphoniaSource},
    SAMPLE_RATE,
};
use rubato::{FastFixedIn, FftFixedIn, PolynomialDegree, Resampler};
use std::{f32::consts::SQRT_2, fs::File, num::NonZeroUsize, path::Path};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};

fn bench_different_downmixing_techniques(c: &mut Criterion) {
    let path = Path::new("data/s32_stereo_44_1_kHz.flac");
    let file = File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let source = SymphoniaSource::new(mss).unwrap();
    let num_channels = source.channels();
    let total_duration = source.total_duration().unwrap();
    let sample_rate = source.sample_rate();
    let samples: Vec<f32> = source.into_iter().collect();

    let mut group = c.benchmark_group("mecomp-analysis: downmixing");

    group.bench_function("with fold", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                let _ = black_box(source.into_iter().enumerate().fold(
                    // pre-allocate the right capacity
                    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                    Vec::with_capacity(
                        (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                    ),
                    // collapse the channels into one channel
                    |mut acc, (i, sample)| {
                        let channel = i % num_channels;
                        #[allow(clippy::cast_precision_loss)]
                        if channel == 0 {
                            acc.push(sample / num_channels as f32);
                        } else {
                            let last_index = acc.len() - 1;
                            acc[last_index] =
                                sample.mul_add(1. / num_channels as f32, acc[last_index]);
                        }
                        acc
                    },
                ));
            },
        );
    });

    group.bench_function("with iterator", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                black_box({
                    let mut mono_sample_array = Vec::with_capacity(
                        (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                    );
                    let mut iter = source.into_iter();
                    while let Some(left) = iter.next() {
                        let right = iter.next().unwrap_or_default();
                        let sum = left + right;
                        let avg = sum * SQRT_2 / 2.0;
                        mono_sample_array.push(avg);
                    }
                })
            },
        );
    });

    group.bench_function("boomer loop", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                let mut mono_sample_array = Vec::with_capacity(
                    (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                );

                for chunk in source.into_iter().collect::<Vec<_>>().chunks_exact(2) {
                    mono_sample_array.push((chunk[0] + chunk[1]) * SQRT_2 / 2.);
                }
            },
        );
    });

    group.bench_function("with chunks", |b| {
        b.iter(|| {
            let source = samples.clone();
            let _: Vec<_> = black_box(
                source
                    .chunks_exact(2)
                    .map(|chunk| (chunk[0] + chunk[1]) * SQRT_2 / 2.)
                    .collect(),
            );
        })
    });

    group.bench_function("side-by-side", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                let mut mono_sample_array = Vec::with_capacity(
                    (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                );
                let mut iter = source.into_iter();
                while let (Some(left), right) = (iter.next(), iter.next().unwrap_or_default()) {
                    mono_sample_array.push(black_box((left + right) * SQRT_2 / 2.))
                }
            },
        );
    });

    group.bench_function("with take", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                black_box({
                    let mut mono_sample_array = Vec::with_capacity(
                        (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                    );
                    let mut iter = source.into_iter().peekable();
                    while iter.peek().is_some() {
                        let sum = iter.by_ref().take(num_channels).sum::<f32>();
                        mono_sample_array.push(sum / (num_channels as f32));
                    }
                })
            },
        );
    });

    group.finish();
}

fn bench_mecomp_decoder_decode(c: &mut Criterion) {
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
                        decoder.decode(black_box(&path)).unwrap();
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_mecomp_decoder_analyze_path(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()
        .unwrap();

    c.bench_function(
        "mecomp-analysis: decoder.rs: MecompDecoder::analyze_path",
        |b| {
            b.iter_with_setup(
                || Decoder::new().unwrap(),
                |decoder| decoder.analyze_path(black_box(&path)).unwrap(),
            );
        },
    );
}

fn bench_mecomp_decoder_analyze_paths(c: &mut Criterion) {
    // get the paths to every music file in the "data" directory
    let mut paths = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .read_dir()
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| {
            p.is_file()
                && (p.extension().unwrap() == "wav"
                    || p.extension().unwrap() == "flac"
                    || p.extension().unwrap() == "mp3")
        })
        .collect::<Vec<_>>();

    // quadruple the number of paths to analyze
    paths.extend(paths.clone());
    paths.extend(paths.clone());

    let mut group = c.benchmark_group("mecomp-analysis: decoder.rs: MecompDecoder::analyze_paths");

    group.bench_function("current default", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths(black_box(&paths));
            },
        );
    });

    let cores = std::thread::available_parallelism().map_or(1usize, NonZeroUsize::get);
    let half = NonZeroUsize::new(cores / 2).unwrap();
    let quarter = NonZeroUsize::new(cores / 4).unwrap();
    let full = NonZeroUsize::new(cores).unwrap();

    group.bench_function("full parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), full);
            },
        );
    });

    group.bench_function("half parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), half);
            },
        );
    });

    group.bench_function("quarter parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), quarter);
            },
        );
    });

    group.finish();
}

criterion_group!(downmixing, bench_different_downmixing_techniques);
criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(20));
    targets = bench_mecomp_decoder_decode,
    bench_mecomp_decoder_analyze_path,
    bench_mecomp_decoder_analyze_paths
);
criterion_main!(benches, downmixing);
