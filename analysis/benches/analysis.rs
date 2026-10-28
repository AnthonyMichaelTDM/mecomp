use criterion::{Criterion, criterion_group, criterion_main};
use decoder::{Decoder as _, MecompDecoder as Decoder};
use mecomp_analysis::{Analysis, decoder};
use std::{hint::black_box, path::Path};

fn bench_analysis_from_samples(c: &mut Criterion) {
    // the same path used in the embeddings benchmarks
    let path = Path::new("data/5_mins_of_noise_stereo_48kHz.ogg");

    let samples = Decoder::new().unwrap().decode(&path).unwrap();

    c.bench_function("mecomp-analysis: lib.rs: Analysis::from_samples", |b| {
        b.iter(|| {
            let _ = black_box(Analysis::from_samples(black_box(&samples)));
        });
    });
}

criterion_group!(benches, bench_analysis_from_samples);
criterion_main!(benches);
