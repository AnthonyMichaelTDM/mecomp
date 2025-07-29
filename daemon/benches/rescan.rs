//! Benchmark of the library rescan function.

use criterion::{Criterion, criterion_group, criterion_main};
use mecomp_daemon::services::library::rescan;
use mecomp_storage::test_utils::ARTIST_NAME_SEPARATOR;
use mecomp_storage::test_utils::SongCase;
use mecomp_storage::test_utils::create_song_metadata;
use mecomp_storage::test_utils::init_test_database;
use mecomp_storage::util::MetadataConflictResolution;
use one_or_many::OneOrMany;

fn benchmark_rescan(c: &mut Criterion) {
    let tempdir = tempfile::tempdir().unwrap();

    // generate song
    let _ = create_song_metadata(
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
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 1,
            artists: vec![1, 2],
            album_artists: vec![2, 3, 4],
            album: 2,
            genre: 1,
        },
    );
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 2,
            artists: vec![1, 2, 3],
            album_artists: vec![2, 3],
            album: 1,
            genre: 2,
        },
    );
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 3,
            artists: vec![2, 3, 4],
            album_artists: vec![2, 3, 4],
            album: 2,
            genre: 2,
        },
    );
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 4,
            artists: vec![3],
            album_artists: vec![2, 3],
            album: 1,
            genre: 1,
        },
    );
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 5,
            artists: vec![2],
            album_artists: vec![2, 3],
            album: 1,
            genre: 2,
        },
    );
    let _ = create_song_metadata(
        &tempdir,
        SongCase {
            song: 6,
            artists: vec![1, 2, 3],
            album_artists: vec![2, 3],
            album: 2,
            genre: 1,
        },
    );

    c.bench_function("mecomp_daemon: rescan", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(async || {
                let db = init_test_database().await.unwrap();
                rescan(
                    &db,
                    &[tempdir.path().to_path_buf()],
                    &ARTIST_NAME_SEPARATOR.to_string().into(),
                    &OneOrMany::None,
                    Some(ARTIST_NAME_SEPARATOR),
                    MetadataConflictResolution::default(),
                )
                .await
                .unwrap();
            });
    });
}

criterion_group!(benches, benchmark_rescan);
criterion_main!(benches);
