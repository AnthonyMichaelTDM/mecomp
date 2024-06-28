use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mecomp_analysis::decoder::Decoder as DecoderTrait;
use mecomp_analysis::decoder::MecompDecoder as Decoder;
use std::path::Path;

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

criterion_group!(
    benches,
    bench_mecomp_decoder_decode,
    bench_mecomp_decoder_analyze_path,
    bench_mecomp_decoder_analyze_paths
);
criterion_main!(benches);
