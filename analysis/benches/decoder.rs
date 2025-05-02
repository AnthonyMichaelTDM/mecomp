use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mecomp_analysis::{
    SAMPLE_RATE,
    decoder::{Decoder as DecoderTrait, MecompDecoder as Decoder, SymphoniaSource},
};
use rubato::{FastFixedIn, FftFixedIn, PolynomialDegree, Resampler};
use std::{f32::consts::SQRT_2, fs::File, num::NonZeroUsize, path::Path};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};

fn bench_different_downmixing_techniques(c: &mut Criterion) {
    let path = Path::new("data/s32_stereo_44_1_kHz.flac");
    let file = File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let source = SymphoniaSource::new(mss).unwrap();
    let num_channels = source.channels();
    let total_duration = source.total_duration().unwrap();
    let sample_rate = source.sample_rate();
    let samples: Vec<f32> = source.into_iter().collect();

    let mut group = c.benchmark_group("mecomp-analysis: downmixing");

    group.bench_function("with fold", |b| {
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

    group.bench_function("with iterator", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
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
            },
        );
    });

    group.bench_function("boomer loop", |b| {
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

    group.bench_function("with chunks", |b| {
        b.iter(|| {
            let source = samples.clone();
            let _: Vec<_> = black_box(
                source
                    .chunks_exact(2)
                    .map(|chunk| (chunk[0] + chunk[1]) * SQRT_2 / 2.)
                    .collect(),
            );
        });
    });

    group.bench_function("side-by-side", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                let mut mono_sample_array = Vec::with_capacity(
                    (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                );
                let mut iter = source.into_iter();
                while let (Some(left), right) = (iter.next(), iter.next().unwrap_or_default()) {
                    mono_sample_array.push(black_box((left + right) * SQRT_2 / 2.));
                }
            },
        );
    });

    group.bench_function("with take", |b| {
        b.iter_with_setup(
            || samples.clone(),
            |source| {
                {
                    let mut mono_sample_array = Vec::with_capacity(
                        (total_duration.as_secs() as usize + 1) * sample_rate as usize,
                    );
                    let mut iter = source.into_iter().peekable();
                    while iter.peek().is_some() {
                        let sum = iter.by_ref().take(num_channels).sum::<f32>();
                        mono_sample_array.push(sum / (num_channels as f32));
                    }
                };
                black_box(());
            },
        );
    });

    group.finish();
}

fn bench_different_resampling_techniques(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("s32_stereo_44_1_kHz.flac")
        .canonicalize()
        .unwrap();
    let mss = MediaSourceStream::new(
        Box::new(File::open(&path).unwrap()),
        MediaSourceStreamOptions::default(),
    );
    let source = SymphoniaSource::new(mss).unwrap();
    let sample_rate = source.sample_rate();
    let total_duration = source.total_duration().unwrap();
    let channels = source.channels();
    assert!(sample_rate == 44100);
    assert!(channels == 2);
    let samples: Vec<f32> = Decoder::into_mono_samples(source.collect(), channels).unwrap();

    let resample_ratio = f64::from(SAMPLE_RATE) / f64::from(sample_rate);

    let mut group = c.benchmark_group("mecomp-analysis: resampling");

    group.bench_function("one-shot FastFixedIn", |b| {
        b.iter(|| {
            let mut resampler = FastFixedIn::new(
                resample_ratio,
                1.0,
                PolynomialDegree::Cubic,
                samples.len(),
                1,
            )
            .unwrap();
            black_box(resampler.process(&[&samples], None).unwrap());
        });
    });

    group.bench_function("chunked FastFixedIn", |b| {
        const CHUNK_SIZE: usize = 4096;

        b.iter(|| {
            let mut resampler =
                FastFixedIn::new(resample_ratio, 1.0, PolynomialDegree::Cubic, CHUNK_SIZE, 1)
                    .unwrap();
            let mut resampled_frames = Vec::with_capacity(
                (usize::try_from(total_duration.as_secs()).unwrap_or(usize::MAX) + 1)
                    * SAMPLE_RATE as usize,
            );

            let delay = resampler.output_delay();

            let new_length = samples.len() * SAMPLE_RATE as usize / sample_rate as usize;
            let mut output_buffer = resampler.output_buffer_allocate(true);

            // chunks of frames, each being CHUNKSIZE long.
            let sample_chunks = samples.chunks_exact(CHUNK_SIZE);
            let remainder = sample_chunks.remainder();

            for chunk in sample_chunks {
                let (_, output_written) = resampler
                    .process_into_buffer(&[chunk], output_buffer.as_mut_slice(), None)
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            // process the remainder
            if !remainder.is_empty() {
                let (_, output_written) = resampler
                    .process_partial_into_buffer(
                        Some(&[remainder]),
                        output_buffer.as_mut_slice(),
                        None,
                    )
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            // flush final samples from resampler
            if resampled_frames.len() < new_length + delay {
                let (_, output_written) = resampler
                    .process_partial_into_buffer(
                        Option::<&[&[f32]]>::None,
                        output_buffer.as_mut_slice(),
                        None,
                    )
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            black_box(resampled_frames[delay..new_length + delay].to_vec());
        });
    });

    group.bench_function("chunked FftFixedIn", |b| {
        const CHUNK_SIZE: usize = 4096;

        b.iter(|| {
            let mut resampler =
                FftFixedIn::new(sample_rate as usize, SAMPLE_RATE as usize, CHUNK_SIZE, 4, 1)
                    .unwrap();

            let mut resampled_frames = Vec::with_capacity(
                (usize::try_from(total_duration.as_secs()).unwrap_or(usize::MAX) + 1)
                    * SAMPLE_RATE as usize,
            );

            let delay = resampler.output_delay();

            let new_length = samples.len() * SAMPLE_RATE as usize / sample_rate as usize;
            let mut output_buffer = resampler.output_buffer_allocate(true);

            // chunks of frames, each being CHUNKSIZE long.
            let sample_chunks = samples.chunks_exact(CHUNK_SIZE);
            let remainder = sample_chunks.remainder();

            for chunk in sample_chunks {
                let (_, output_written) = resampler
                    .process_into_buffer(&[chunk], output_buffer.as_mut_slice(), None)
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            // process the remainder
            if !remainder.is_empty() {
                let (_, output_written) = resampler
                    .process_partial_into_buffer(
                        Some(&[remainder]),
                        output_buffer.as_mut_slice(),
                        None,
                    )
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            // flush final samples from resampler
            if resampled_frames.len() < new_length + delay {
                let (_, output_written) = resampler
                    .process_partial_into_buffer(
                        Option::<&[&[f32]]>::None,
                        output_buffer.as_mut_slice(),
                        None,
                    )
                    .unwrap();
                resampled_frames.extend_from_slice(&output_buffer[0][..output_written]);
            }

            black_box(resampled_frames[delay..new_length + delay].to_vec());
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
            b.iter_with_setup(
                || Decoder::new().unwrap(),
                |decoder| decoder.analyze_path(black_box(&path)).unwrap(),
            );
        },
    );
}

fn bench_mecomp_decoder_analyze_paths(c: &mut Criterion) {
    // get the paths to every music file in the "data" directory
    let mut paths = Path::new(env!("CARGO_MANIFEST_DIR"))
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

    // quadruple the number of paths to analyze
    paths.extend(paths.clone());
    paths.extend(paths.clone());

    let mut group = c.benchmark_group("mecomp-analysis: decoder.rs: MecompDecoder::analyze_paths");

    group.bench_function("current default", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths(black_box(&paths));
            },
        );
    });

    let cores = std::thread::available_parallelism().map_or(1usize, NonZeroUsize::get);
    let half = NonZeroUsize::new(cores / 2).unwrap();
    let quarter = NonZeroUsize::new(cores / 4).unwrap();
    let full = NonZeroUsize::new(cores).unwrap();

    group.bench_function("full parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), full);
            },
        );
    });

    group.bench_function("half parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), half);
            },
        );
    });

    group.bench_function("quarter parallelism", |b| {
        b.iter_with_setup(
            || Decoder::new().unwrap(),
            |decoder| {
                decoder.analyze_paths_with_cores(black_box(&paths), quarter);
            },
        );
    });

    group.finish();
}

criterion_group!(
    decoder_stages,
    bench_different_downmixing_techniques,
    bench_different_resampling_techniques,
);
criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(20));
    targets = bench_mecomp_decoder_analyze_path,
);
criterion_group!(
    name = analyze_paths;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(20)).sample_size(30);
    targets = bench_mecomp_decoder_analyze_paths
);
criterion_main!(decoder_stages, benches, analyze_paths);
