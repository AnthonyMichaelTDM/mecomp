//! Benchmarks for the full analysis pipeline
use criterion::{Criterion, criterion_group, criterion_main};
use std::path::Path;
use std::time::Duration;

use mecomp_analysis::decoder::{Decoder, MecompDecoder};

fn bench_analysis_pipeline(c: &mut Criterion) {
    let cases = [
        ("s16_mono_22_5kHz.flac", "mono", 10),
        ("s16_stereo_22_5kHz.flac", "stereo", 10),
        ("s32_mono_44_1_kHz.flac", "resample_mono", 10),
        ("s32_stereo_44_1_kHz.flac", "resample_multi", 10),
        ("s32_stereo_44_1_kHz.mp3", "mp3", 10),
        ("5_mins_of_noise_stereo_48kHz.ogg", "long_song", 30),
    ]
    .iter()
    .map(|&(file, case, measurement_time)| {
        (
            Path::new("data").join(file),
            case,
            Duration::from_secs(measurement_time),
        )
    });

    let mut group = c.benchmark_group("mecomp_analysis: analysis_pipeline");

    let decoder = MecompDecoder::new().unwrap();

    for (path, case, measurement_time) in cases {
        group
            .measurement_time(measurement_time)
            .bench_with_input(case, &path, |b, path| {
                b.iter_with_large_drop(|| {
                    let _ = decoder.analyze_path(path).unwrap();
                });
            });
    }

    group.finish();
}

criterion_group!(analysis_pipeline, bench_analysis_pipeline);
criterion_main!(analysis_pipeline);
