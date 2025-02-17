//! Benchmark for recluster
//!
//! because it actually cares about what the audio features are, we'll be running this on
//! my real music library

use criterion::{criterion_group, criterion_main, Criterion};
use mecomp_core::config::{ClusterAlgorithm, ReclusterSettings};
use mecomp_daemon::services::library::recluster;
use mecomp_storage::db::schemas::analysis::Analysis;
use mecomp_storage::db::schemas::collection::{Collection, TABLE_NAME};
use mecomp_storage::db::schemas::song::Song;
use mecomp_storage::test_utils::{
    arb_analysis_features, arb_song_case, arb_vec, create_song_metadata, init_test_database,
    SongCase,
};
use tokio::runtime::Runtime;

fn benchmark_recluster(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = rt.block_on(init_test_database()).unwrap();

    let settings = ReclusterSettings {
        gap_statistic_reference_datasets: 50,
        max_clusters: 16,
        algorithm: ClusterAlgorithm::GMM,
    };

    // load some songs into the database
    let song_cases = arb_vec(&arb_song_case(), 100..=150)();
    let song_cases = song_cases.into_iter().enumerate().map(|(i, sc)| SongCase {
        song: u8::try_from(i).unwrap_or(u8::MAX),
        ..sc
    });
    let metadatas = song_cases
        .into_iter()
        .map(|song_case| create_song_metadata(&dir, song_case))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut songs = Vec::with_capacity(metadatas.len());
    for metadata in &metadatas {
        songs.push(
            rt.block_on(Song::try_load_into_db(&db, metadata.clone()))
                .unwrap(),
        );
    }

    // load some dummy analyses into the database
    for song in &songs {
        rt.block_on(Analysis::create(
            &db,
            song.id.clone(),
            Analysis {
                id: Analysis::generate_id(),
                features: arb_analysis_features()(),
            },
        ))
        .unwrap();
    }

    c.bench_function("mecomp_daemon: recluster (gmm)", |b| {
        b.to_async(Runtime::new().unwrap()).iter_with_setup(
            || async {
                let _: Vec<Collection> = db.delete(TABLE_NAME).await.unwrap();
                db.clone()
            },
            |db| async move {
                let db = db.await;
                recluster(&db, &settings).await.unwrap();
            },
        );
    });

    let settings = ReclusterSettings {
        algorithm: ClusterAlgorithm::KMeans,
        ..settings
    };

    c.bench_function("mecomp_daemon: recluster (kmeans)", |b| {
        b.to_async(Runtime::new().unwrap()).iter_with_setup(
            || async {
                let _: Vec<Collection> = db.delete(TABLE_NAME).await.unwrap();
                db.clone()
            },
            |db| async move {
                let db = db.await;
                recluster(&db, &settings).await.unwrap();
            },
        );
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = benchmark_recluster
);
criterion_main!(benches);
