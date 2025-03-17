//! Benchmark of the library analysis function.

use std::num::NonZeroUsize;

use criterion::{criterion_group, criterion_main, Criterion};
use mecomp_daemon::services::library::analyze;
use mecomp_storage::{
    db::schemas::song::Song,
    test_utils::{arb_song_case, arb_vec, create_song_metadata, init_test_database},
};
use tokio::runtime::Runtime;

fn benchmark_analyze(c: &mut Criterion) {
    let tempdir = tempfile::tempdir().unwrap();

    // we want to test how this works when all threads are being utilized, so we need a lot of songs
    let num_songs = std::thread::available_parallelism().map_or(1, NonZeroUsize::get) * 2;

    // generate song
    let songs = arb_vec(&arb_song_case(), num_songs..=num_songs)();
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
    config = Criterion::default().sample_size(30).measurement_time(std::time::Duration::from_secs(30));
    targets = benchmark_analyze
);
criterion_main!(benches);
