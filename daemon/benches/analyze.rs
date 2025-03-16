//! Benchmark of the library analysis function.

use criterion::{criterion_group, criterion_main, Criterion};
use mecomp_daemon::services::library::analyze;
use mecomp_storage::db::schemas::song::Song;
use mecomp_storage::test_utils::init_test_database;
use mecomp_storage::test_utils::{arb_song_case, arb_vec, create_song_metadata};
use tokio::runtime::Runtime;

fn benchmark_rescan(c: &mut Criterion) {
    let tempdir = tempfile::tempdir().unwrap();

    // generate song
    let songs = arb_vec(&arb_song_case(), 7..=7)();
    let songs = songs
        .into_iter()
        .map(|song_case| create_song_metadata(&tempdir, song_case).unwrap())
        .collect::<Vec<_>>();

    // each iteration needs to remove the analyses it creates, but we don't want to include that in the benchmark timings
    c.bench_function("mecomp_daemon: analyze", |b| {
        b.to_async(Runtime::new().unwrap()).iter_with_setup(
            || {
                let songs = songs.clone();
                let handle = tokio::runtime::Handle::current();
                std::thread::spawn(move || {
                    handle.block_on(async move {
                        let db = init_test_database().await.unwrap();

                        for song in songs {
                            let _ = Song::try_load_into_db(&db, song).await.unwrap();
                        }

                        db
                    })
                })
                .join()
                .unwrap()
            },
            |db| async move {
                analyze(&db, false).await.unwrap();
            },
        );
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(30);
    targets = benchmark_rescan
);
criterion_main!(benches);
