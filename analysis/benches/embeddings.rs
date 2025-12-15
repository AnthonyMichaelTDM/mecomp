use criterion::{Criterion, criterion_group, criterion_main};
use decoder::{Decoder as _, MecompDecoder as Decoder};
use mecomp_analysis::{decoder, embeddings::AudioEmbeddingModel};
use std::{hint::black_box, path::Path};

fn bench_load_embeddings_model(c: &mut Criterion) {
    c.bench_function("embeddings.rs: AudioEmbeddingModel::load_default", |b| {
        b.iter(|| {
            let _ = black_box(AudioEmbeddingModel::load_default().unwrap());
        });
    });

    c.bench_function("embeddings.rs: AudioEmbeddingModel::load_from_onnx", |b| {
        let model_path = Path::new("models/audio_embedding_model.onnx");
        b.iter(|| {
            let _ = black_box(AudioEmbeddingModel::load_from_onnx(black_box(&model_path)).unwrap());
        });
    });
}

fn bench_generate_embeddings(c: &mut Criterion) {
    let path = Path::new("data/5_mins_of_noise_stereo_48kHz.ogg");

    let samples = Decoder::new().unwrap().decode(&path).unwrap();
    // only use the first 20% of the samples so the bandwidth of the benchmarks is the same
    let samples = mecomp_analysis::ResampledAudio {
        samples: samples.samples[..samples.samples.len() / 5].to_vec(),
        path: samples.path,
    };

    let batch_of_samples = vec![samples.clone(); 20];

    let mut model = AudioEmbeddingModel::load_default().unwrap();

    c.bench_function("embeddings.rs: AudioEmbeddingModel::embed", |b| {
        b.iter(|| {
            let _ = black_box(model.embed(black_box(&samples)).unwrap());
        });
    });

    c.bench_function("embeddings.rs: AudioEmbeddingModel::embed_batch", |b| {
        b.iter(|| {
            let _ = black_box(model.embed_batch(black_box(&batch_of_samples)).unwrap());
        });
    });
}

criterion_group!(
    benches,
    bench_load_embeddings_model,
    bench_generate_embeddings
);
criterion_main!(benches);
