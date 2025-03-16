use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::{
    decoder::{Decoder as _, MecompDecoder as Decoder},
    misc::LoudnessDesc,
};

fn bench_loudness_desc(c: &mut Criterion) {
    let loudness_desc = LoudnessDesc::default();
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

    c.bench_function("mecomp-analysis: misc.rs: LoudnessDesc", |b| {
        b.iter(|| {
            let mut loudness_desc = loudness_desc.clone();
            loudness_desc.do_(black_box(&signal));
            loudness_desc.get_value();
        });
    });
}

criterion_group!(benches, bench_loudness_desc);
criterion_main!(benches);
