use criterion::{Criterion, criterion_group, criterion_main};
use mecomp_analysis::{
    decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder},
    utils::stft,
};
use ndarray::{Array1, Array2, arr2};
use ndarray_npy::ReadNpyExt;
use std::{fs::File, hint::black_box, path::Path};

use mecomp_analysis::chroma::{
    chroma_filter, chroma_stft, estimate_tuning, normalize_feature_sequence, pip_track,
    pitch_tuning,
};

fn bench_estimate_tuning(c: &mut Criterion) {
    let file = File::open("data/spectrum-chroma.npy").unwrap();
    let arr = Array2::<f64>::read_npy(file).unwrap();

    c.bench_function("mecomp-analysis: chroma.rs: estimate_tuning", |b| {
        b.iter(|| {
            estimate_tuning(
                black_box(22050),
                black_box(&arr),
                black_box(2048),
                black_box(0.01),
                black_box(12),
            )
            .unwrap();
        });
    });
}

fn bench_pitch_tuning(c: &mut Criterion) {
    let file = File::open("data/pitch-tuning.npy").unwrap();
    let pitch = Array1::<f64>::read_npy(file).unwrap();

    c.bench_function("mecomp-analysis: chroma.rs: pitch_tuning", |b| {
        b.iter(|| {
            pitch_tuning(
                black_box(&mut pitch.to_owned()),
                black_box(0.05),
                black_box(12),
            )
            .unwrap();
        });
    });
}

fn bench_pip_track(c: &mut Criterion) {
    let file = File::open("data/spectrum-chroma.npy").unwrap();
    let spectrum = Array2::<f64>::read_npy(file).unwrap();

    c.bench_function("mecomp-analysis: chroma.rs: pip_track", |b| {
        b.iter(|| {
            pip_track(black_box(22050), black_box(&spectrum), black_box(2048)).unwrap();
        });
    });
}

fn bench_chroma_filter(c: &mut Criterion) {
    c.bench_function("mecomp-analysis: chroma.rs: chroma_filter", |b| {
        b.iter(|| {
            chroma_filter(
                black_box(22050),
                black_box(2048),
                black_box(12),
                black_box(-0.1),
            )
            .unwrap();
        });
    });
}

fn bench_normalize_feature_sequence(c: &mut Criterion) {
    let array = arr2(&[[0.1, 0.3, 0.4], [1.1, 0.53, 1.01]]);
    c.bench_function(
        "mecomp-analysis: chroma.rs: normalize_feature_sequence",
        |b| {
            b.iter(|| {
                normalize_feature_sequence(black_box(&array));
            });
        },
    );
}

fn bench_chroma_stft(c: &mut Criterion) {
    let signal = Decoder::new()
        .unwrap()
        .decode(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets/music.mp3")
                .canonicalize()
                .unwrap(),
        )
        .unwrap()
        .samples;
    let stft = stft(&signal, 8192, 2205);

    c.bench_function("mecomp-analysis: chroma.rs: chroma_stft", |b| {
        b.iter_batched(
            || stft.clone(),
            |stft| {
                chroma_stft(
                    black_box(22050),
                    black_box(&stft),
                    black_box(8192),
                    black_box(12),
                    black_box(-0.049_999_999_999_999_99),
                )
                .unwrap();
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group!(
    benches,
    bench_estimate_tuning,
    bench_pitch_tuning,
    bench_pip_track,
    bench_chroma_filter,
    bench_normalize_feature_sequence,
    bench_chroma_stft,
);
criterion_main!(benches);
