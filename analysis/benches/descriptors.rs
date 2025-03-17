//! Benchmarks for the descriptors

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::{
    chroma::ChromaDesc,
    decoder::{Decoder as _, MecompDecoder as Decoder},
    misc::LoudnessDesc,
    temporal::BPMDesc,
    timbral::{SpectralDesc, ZeroCrossingRateDesc},
    SAMPLE_RATE,
};

fn bench_descriptors(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("mecomp-analysis: descriptors");

    group.bench_with_input("temporal.rs: BMPDesc", &signal, |b, signal| {
        b.iter_with_setup(
            || BPMDesc::new(SAMPLE_RATE).unwrap(),
            |mut tempo_desc| {
                let windows = signal
                    .windows(BPMDesc::WINDOW_SIZE)
                    .step_by(BPMDesc::HOP_SIZE);
                for window in windows {
                    tempo_desc.do_(black_box(window)).unwrap();
                }
                black_box(tempo_desc.get_value());
            },
        );
    });

    group.bench_with_input("timbral.rs: ZeroCrossingRateDesc", &signal, |b, signal| {
        b.iter_with_setup(
            || ZeroCrossingRateDesc::default(),
            |mut zcr_desc| {
                zcr_desc.do_(black_box(signal));
                black_box(zcr_desc.get_value());
            },
        );
    });

    group.bench_with_input("timbral.rs: SpectralDesc", &signal, |b, signal| {
        b.iter_with_setup(
            || SpectralDesc::new(SAMPLE_RATE).unwrap(),
            |mut spectral_desc| {
                spectral_desc.do_(black_box(signal)).unwrap();
                let windows = signal
                    .windows(SpectralDesc::WINDOW_SIZE)
                    .step_by(SpectralDesc::HOP_SIZE);
                for window in windows {
                    spectral_desc.do_(black_box(window)).unwrap();
                }

                black_box(spectral_desc.get_centroid());
                black_box(spectral_desc.get_rolloff());
                black_box(spectral_desc.get_flatness());
            },
        );
    });

    group.bench_with_input("misc.rs: LoudnessDesc", &signal, |b, signal| {
        b.iter_with_setup(
            || LoudnessDesc::default(),
            |mut loudness_desc| {
                loudness_desc.do_(black_box(signal));
                let windows = signal.chunks(LoudnessDesc::WINDOW_SIZE);
                for window in windows {
                    loudness_desc.do_(black_box(window));
                }

                black_box(loudness_desc.get_value());
            },
        );
    });

    group.bench_with_input("chroma.rs: ChromaDesc", &signal, |b, signal| {
        b.iter_with_setup(
            || ChromaDesc::new(SAMPLE_RATE, 12),
            |mut chroma_desc| {
                chroma_desc.do_(black_box(signal)).unwrap();
                black_box(chroma_desc.get_value());
            },
        );
    });

    group.finish();
}

criterion_group!(benches, bench_descriptors);
criterion_main!(benches);
