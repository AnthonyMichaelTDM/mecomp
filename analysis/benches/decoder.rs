use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::decoder::Decoder as DecoderTrait;
use mecomp_analysis::decoder::MecompDecoder as Decoder;
use mecomp_analysis::decoder::SymphoniaSource;
use std::f32::consts::SQRT_2;
use std::fs::File;
use std::path::Path;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::io::MediaSourceStreamOptions;

fn bench_different_downmixing_techniques(c: &mut Criterion) {
    let path = Path::new("data/s32_stereo_44_1_kHz.flac");
    let file = File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let source = SymphoniaSource::new(mss).unwrap();
    let num_channels = source.channels();
    let total_duration = source.total_duration().unwrap();
    let sample_rate = source.sample_rate();
    let samples: Vec<f32> = source.into_iter().collect();

    c.bench_function("mecomp-analysis: downmixing with fold", |b| {
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

    c.bench_function("mecomp-analysis: downmixing with iterator", |b| {
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

    c.bench_function("mecomp_analysis: downmixing boomer loop", |b| {
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

    c.bench_function("mecomp_analysis: downmixing with chunks", |b| {
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

    c.bench_function("mecomp_analysis: downmixing side-by-side", |b| {
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

    c.bench_function("mecomp_analysis: downmixing with take", |b| {
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
}

fn bench_mecomp_decoder_decode(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()
        .unwrap();

    c.bench_function("mecomp-analysis: decoder.rs: MecompDecoder::decode", |b| {
        b.iter(|| {
            let _ = black_box(Decoder::decode(black_box(&path)));
        });
    });
}

fn bench_mecomp_decoder_analyze_path(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()
        .unwrap();

    c.bench_function(
        "mecomp-analysis: decoder.rs: MecompDecoder::analyze_path",
        |b| {
            b.iter(|| {
                let _ = black_box(Decoder::analyze_path(black_box(&path)));
            });
        },
    );
}

fn bench_mecomp_decoder_analyze_paths(c: &mut Criterion) {
    // get the paths to every music file in the "data" directory
    let paths = Path::new(env!("CARGO_MANIFEST_DIR"))
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

    c.bench_function(
        "mecomp-analysis: decoder.rs: MecompDecoder::analyze_paths",
        |b| {
            b.iter(|| {
                let _ = black_box(Decoder::analyze_paths(black_box(&paths)));
            });
        },
    );
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
