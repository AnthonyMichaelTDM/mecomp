use criterion::{Criterion, black_box, criterion_group, criterion_main};
use decoder::{Decoder as _, MecompDecoder as Decoder};
use mecomp_analysis::{Analysis, decoder};
use std::path::Path;

fn bench_analysis_from_samples(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/music.mp3")
        .canonicalize()
        .unwrap();

    let samples = Decoder::new().unwrap().decode(&path).unwrap();

    c.bench_function("mecomp-analysis: lib.rs: Analysis::from_samples", |b| {
        b.iter(|| {
            let _ = black_box(Analysis::from_samples(black_box(&samples)));
        });
    });
}

criterion_group!(benches, bench_analysis_from_samples);
criterion_main!(benches);
