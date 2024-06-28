use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::{
    decoder::{Decoder, MecompDecoder},
    timbral::{SpectralDesc, ZeroCrossingRateDesc},
};

fn bench_spectral_desc(c: &mut Criterion) {
    let signal = MecompDecoder::decode(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/music.mp3")
            .canonicalize()
            .unwrap(),
    )
    .unwrap()
    .samples;

    c.bench_function("mecomp-analysis: timbral.rs: SpectralDesc", |b| {
        b.iter(|| {
            let mut spectral_desc = SpectralDesc::new(10).unwrap();
            spectral_desc.do_(black_box(&signal)).unwrap();
        });
    });
    c.bench_function(
        "mecomp-analysis: timbral.rs: SpectralDesc::get_centroid ",
        |b| {
            b.iter(|| {
                let mut spectral_desc = SpectralDesc::new(10).unwrap();
                spectral_desc.do_(black_box(&signal)).unwrap();
                spectral_desc.get_centroid();
            });
        },
    );
    c.bench_function(
        "mecomp-analysis: timbral.rs: SpectralDesc::get_rolloff",
        |b| {
            b.iter(|| {
                let mut spectral_desc = SpectralDesc::new(10).unwrap();
                spectral_desc.do_(black_box(&signal)).unwrap();
                spectral_desc.get_rolloff();
            });
        },
    );
    c.bench_function(
        "mecomp-analysis: timbral.rs: SpectralDesc::get_flatness",
        |b| {
            b.iter(|| {
                let mut spectral_desc = SpectralDesc::new(10).unwrap();
                spectral_desc.do_(black_box(&signal)).unwrap();
                spectral_desc.get_flatness();
            });
        },
    );
}

fn bench_zcr_desc(c: &mut Criterion) {
    let zcr_desc = ZeroCrossingRateDesc::new(10);
    let signal = MecompDecoder::decode(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/music.mp3")
            .canonicalize()
            .unwrap(),
    )
    .unwrap()
    .samples;

    c.bench_function("mecomp-analysis: timbral.rs: ZeroCrossingRateDesc", |b| {
        b.iter(|| {
            let mut zcr_desc = zcr_desc.clone();
            zcr_desc.do_(black_box(&signal));
            zcr_desc.get_value();
        });
    });
}

criterion_group!(benches, bench_spectral_desc, bench_zcr_desc);
criterion_main!(benches);
