use criterion::{Criterion, criterion_group, criterion_main};
use mecomp_storage::db::schemas::song::Song;
use mecomp_storage::test_utils::SongCase;
use mecomp_storage::test_utils::create_song_metadata;
use mecomp_storage::test_utils::init_test_database;

fn benchmark_try_load_into_db(c: &mut Criterion) {
    let tempdir = tempfile::tempdir().unwrap();

    let metadata = create_song_metadata(
        &tempdir,
        SongCase {
            song: 0,
            artists: vec![1, 2, 3],
            album_artists: vec![2, 3],
            album: 1,
            genre: 1,
        },
    )
    .unwrap();

    c.bench_function("mecomp_storage: Song::try_load_into_db", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(async || {
                let db = init_test_database().await.unwrap();
                let _song = Song::try_load_into_db(&db, metadata.clone()).await.unwrap();
            });
    });
}

criterion_group!(benches, benchmark_try_load_into_db);
criterion_main!(benches);
