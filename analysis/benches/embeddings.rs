use criterion::{Criterion, criterion_group, criterion_main};
use mecomp_analysis::{
    decoder::{Decoder as _, MecompDecoder as Decoder},
    embeddings::{AudioEmbeddingModel, ModelConfig, onnx_test_env},
};
use std::{hint::black_box, path::Path};

fn bench_load_embeddings_model(c: &mut Criterion) {
    c.bench_function("embeddings.rs: AudioEmbeddingModel::load_default", |b| {
        let environment = onnx_test_env();
        b.iter(|| {
            let _ = black_box(AudioEmbeddingModel::load_default(&environment, false).unwrap());
        });
    });

    c.bench_function("embeddings.rs: AudioEmbeddingModel::load_from_onnx", |b| {
        let environment = onnx_test_env();
        let model_path = Path::new("models/audio_embedding_model.onnx");
        b.iter(|| {
            let _ = black_box(
                AudioEmbeddingModel::load_from_onnx(&environment, black_box(&model_path), false)
                    .unwrap(),
            );
        });
    });
}

fn bench_generate_embeddings(c: &mut Criterion) {
    let path = Path::new("data/5_mins_of_noise_stereo_48kHz.ogg");

    let samples = Decoder::new().unwrap().decode(&path).unwrap();
    // only use the first 20% of the samples so the bandwidth of the benchmarks is the same
    let batch_samples = mecomp_analysis::ResampledAudio {
        samples: samples.samples[..samples.samples.len() / 5].to_vec(),
        path: samples.path.clone(),
    };

    let batch_of_samples = vec![batch_samples.clone(); 5];

    let environment = onnx_test_env();
    let mut model = AudioEmbeddingModel::load_default(&environment, false).unwrap();

    c.bench_function("embeddings.rs: AudioEmbeddingModel::embed cpu", |b| {
        b.iter(|| {
            let _ = black_box(model.embed(black_box(&samples)).unwrap());
        });
    });

    c.bench_function("embeddings.rs: AudioEmbeddingModel::embed_batch cpu", |b| {
        b.iter(|| {
            let _ = black_box(model.embed_batch(black_box(&batch_of_samples)).unwrap());
        });
    });

    let mut model = AudioEmbeddingModel::load_default(&environment, true).unwrap();
    c.bench_function(
        "embeddings.rs: AudioEmbeddingModel::embed accelerator",
        |b| {
            b.iter(|| {
                let _ = black_box(model.embed(black_box(&samples)).unwrap());
            });
        },
    );
    c.bench_function(
        "embeddings.rs: AudioEmbeddingModel::embed_batch accelerator",
        |b| {
            b.iter(|| {
                let _ = black_box(model.embed_batch(black_box(&batch_of_samples)).unwrap());
            });
        },
    );
}

fn bench_process_songs(c: &mut Criterion) {
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
        // repeat 4 times
        .flat_map(|p| vec![p.clone(); 4])
        .collect::<Vec<_>>();

    // create a channel for sending and receiving processed songs
    let (tx, rx) = std::sync::mpsc::sync_channel(2 * paths.len());

    // spawn a thread to drain the receiver
    std::thread::spawn(move || for _ in rx {});

    c.bench_function(
        "mecomp-analysis: decoder.rs: MecompDecoder::process_songs",
        |b| {
            b.iter_with_setup(
                || {
                    let decoder = Decoder::new().unwrap();
                    let config: ModelConfig = ModelConfig::default();
                    let tx = tx.clone();
                    let env: ort::environment::Environment = onnx_test_env();
                    (decoder, config, tx, env)
                },
                |(decoder, config, tx, env)| {
                    decoder
                        .process_songs(
                            black_box(&paths),
                            black_box(tx),
                            black_box(config),
                            black_box(&env),
                        )
                        .unwrap();
                },
            );
        },
    );

    c.bench_function(
        "mecomp-analysis: decoder.rs: MecompDecoder::process_songs (accelerator)",
        |b| {
            b.iter_with_setup(
                || {
                    let decoder = Decoder::new().unwrap();
                    let mut config: ModelConfig = ModelConfig::default();
                    config.hardware_acceleration = true;
                    let tx = tx.clone();
                    let env: ort::environment::Environment = onnx_test_env();
                    (decoder, config, tx, env)
                },
                |(decoder, config, tx, env)| {
                    decoder
                        .process_songs(
                            black_box(&paths),
                            black_box(tx),
                            black_box(config),
                            black_box(&env),
                        )
                        .unwrap();
                },
            );
        },
    );

    drop(tx);
}

criterion_group!(load_benches, bench_load_embeddings_model);
criterion_group!(
    name = inference_benches;
    config = Criterion::default().sample_size(10);
    targets = bench_generate_embeddings
);
criterion_group!(
    name = decoder_benches;
    config = Criterion::default().sample_size(10);
    targets = bench_process_songs
);
criterion_main!(load_benches, inference_benches, decoder_benches);
