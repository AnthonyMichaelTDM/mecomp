use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::{
    decoder::{Decoder, MecompDecoder},
    temporal::BPMDesc,
    SAMPLE_RATE,
};

fn bench_bpm_desc(c: &mut Criterion) {
    let signal = MecompDecoder::decode(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/music.mp3")
            .canonicalize()
            .unwrap(),
    )
    .unwrap()
    .samples;

    c.bench_function("mecomp-analysis: temporal.rs: BPMDesc", |b| {
        b.iter(|| {
            let mut tempo_desc = BPMDesc::new(SAMPLE_RATE).unwrap();
            let _ = tempo_desc.do_(black_box(&signal));
            tempo_desc.get_value();
        });
    });
}

criterion_group!(benches, bench_bpm_desc);
criterion_main!(benches);
