use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use log::{debug, error, info, warn};
use mecomp_analysis::{
    clustering::{extract_clusters, KMeansHelper, KOptimal, NotInitialized},
    decoder::{DecoderWithCallback, MecompDecoder},
};
use mecomp_core::state::library::{LibraryBrief, LibraryFull, LibraryHealth};
use surrealdb::{Connection, Surreal};
use tap::TapFallible;
use tracing::instrument;
use walkdir::WalkDir;

use mecomp_storage::{
    db::{
        health::{
            count_albums, count_artists, count_collections, count_orphaned_albums,
            count_orphaned_artists, count_orphaned_collections, count_orphaned_playlists,
            count_playlists, count_songs, count_unanalyzed_songs,
        },
        schemas::{
            album::Album,
            analysis::Analysis,
            artist::Artist,
            collection::Collection,
            playlist::Playlist,
            song::{Song, SongMetadata},
        },
    },
    errors::Error,
    util::MetadataConflictResolution,
};

use crate::config::ReclusterSettings;

/// Index the library.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
/// or if there is an error reading from the file system.
/// or if there is an error writing to the database.
#[instrument]
pub async fn rescan<C: Connection>(
    db: &Surreal<C>,
    paths: &[PathBuf],
    artist_name_separator: Option<&str>,
    genre_separator: Option<&str>,
    conflict_resolution_mode: MetadataConflictResolution,
) -> Result<(), Error> {
    // get all the songs in the current library
    let songs = Song::read_all(db).await?;
    let mut paths_to_skip = HashSet::new(); // use a hashset because hashing is faster than linear search, especially for large libraries

    // for each song, check if the file still exists
    for song in songs {
        let path = song.path.clone();
        if !path.exists() {
            // remove the song from the library
            warn!("Song {} no longer exists, deleting", path.to_string_lossy());
            Song::delete(db, song.id).await?;
            continue;
        }

        debug!("loading metadata for {}", path.to_string_lossy());
        // check if the metadata of the file is the same as the metadata in the database
        match (
            SongMetadata::load_from_path(path.clone(), artist_name_separator, genre_separator),
            conflict_resolution_mode,
        ) {
            // if we have metadata and the metadata is different from the song's metadata, and we are in "overwrite" mode, update the song's metadata
            (Ok(metadata), MetadataConflictResolution::Overwrite)
                if metadata != SongMetadata::from(&song) =>
            {
                info!(
                    "{} has conflicting metadata with index, resolving conflict",
                    path.to_string_lossy()
                );
                // if the file has been modified, update the song's metadata
                Song::update(db, song.id.clone(), metadata.merge_with_song(&song)).await?;
            }
            // if we have metadata and the metadata is different from the song's metadata, and we are in "skip" mode, do nothing
            (Ok(metadata), MetadataConflictResolution::Skip)
                if metadata != SongMetadata::from(&song) =>
            {
                warn!(
                    "{} has conflicting metadata with index, but conflict resolution mode is \"skip\", so we do nothing",
                    path.to_string_lossy()
                );
                continue;
            }
            // if we have an error, delete the song from the library
            (Err(e), _) => {
                warn!(
                    "Error reading metadata for {}: {}",
                    path.to_string_lossy(),
                    e
                );
                info!("assuming the file isn't a song or doesn't exist anymore, removing from library");
                Song::delete(db, song.id).await?;
            }
            // if the metadata is the same, do nothing
            _ => {}
        }

        // now, add the path to the list of paths to skip so that we don't index the song again
        paths_to_skip.insert(path);
    }
    // now, index all the songs in the library that haven't been indexed yet
    let mut visited_paths = paths_to_skip;

    debug!("Indexing paths: {:?}", paths);
    for path in paths
        .iter()
        .filter_map(|p| {
            p.canonicalize()
                .tap_err(|e| warn!("Error canonicalizing path: {}", e))
                .ok()
        })
        .flat_map(|x| WalkDir::new(x).into_iter())
        .filter_map(|x| x.tap_err(|e| warn!("Error reading path: {}", e)).ok())
        .filter_map(|x| x.file_type().is_file().then_some(x))
    {
        if visited_paths.contains(path.path()) {
            continue;
        }

        visited_paths.insert(path.path().to_owned());

        // if the file is a song, add it to the library
        match SongMetadata::load_from_path(
            path.path().to_owned(),
            artist_name_separator,
            genre_separator,
        ) {
            Ok(metadata) => match Song::try_load_into_db(db, metadata).await {
                Ok(_) => {
                    debug!("Indexed {}", path.path().to_string_lossy());
                }
                Err(e) => {
                    warn!("Error indexing {}: {}", path.path().to_string_lossy(), e);
                }
            },
            Err(e) => {
                warn!(
                    "Error reading metadata for {}: {}",
                    path.path().to_string_lossy(),
                    e
                );
            }
        }
    }

    info!("Library rescan complete");
    info!("Library brief: {:?}", brief(db).await?);

    Ok(())
}

/// Analyze the library.
///
/// In order, this function will:
/// - get all the songs that aren't currently analyzed.
/// - start analyzing those songs in batches.
/// - update the database with the analyses.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
///
/// # Panics
///
/// This function will panic if the thread(s) that analyzes the songs panics.
#[instrument]
pub async fn analyze<C: Connection>(db: &Surreal<C>) -> Result<(), Error> {
    // get all the songs that don't have an analysis
    let songs_to_analyze: Vec<Song> = Analysis::read_songs_without_analysis(db).await?;
    // crate a hashmap mapping paths to song ids
    let paths = songs_to_analyze
        .iter()
        .map(|song| (song.path.clone(), song.id.clone()))
        .collect::<HashMap<_, _>>();

    let keys = paths.keys().cloned().collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel();

    // analyze the songs in batches
    let handle = std::thread::spawn(move || {
        MecompDecoder::analyze_paths_with_callback(keys, tx);
    });

    for (song_path, maybe_analysis) in rx {
        let Some(song_id) = paths.get(&song_path) else {
            error!("No song id found for path: {}", song_path.to_string_lossy());
            return Ok(());
        };

        match maybe_analysis {
            Ok(analysis) => {
                if Analysis::create(
                    db,
                    song_id.clone(),
                    Analysis {
                        id: Analysis::generate_id(),
                        features: *analysis.inner(),
                    },
                )
                .await?
                .is_some()
                {
                    debug!("Analyzed {}", song_path.to_string_lossy());
                } else {
                    warn!(
                        "Error analyzing {}: song either wasn't found or already has an analysis",
                        song_path.to_string_lossy()
                    );
                }
            }
            Err(e) => {
                error!("Error analyzing {}: {}", song_path.to_string_lossy(), e);
            }
        }
    }

    handle.join().expect("Couldn't join thread");

    info!("Library analysis complete");
    info!("Library brief: {:?}", brief(db).await?);

    Ok(())
}

/// Recluster the library.
///
/// This function will remove and recompute all the "collections" (clusters) in the library.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
#[instrument]
pub async fn recluster<C: Connection>(
    db: &Surreal<C>,
    settings: &ReclusterSettings,
) -> Result<(), Error> {
    // collect all the analyses
    let samples = Analysis::read_all(db).await?;

    // use k-means to cluster the analyses
    let kmeans: KMeansHelper<NotInitialized<_>> = KMeansHelper::new(
        samples,
        settings.max_clusters,
        KOptimal::GapStatistic {
            b: settings.gap_statistic_reference_datasets,
        },
    );

    let kmeans = match kmeans.initialize() {
        Err(e) => {
            error!("There was an error initializing the k-means helper: {e}",);
            return Ok(());
        }
        Ok(kmeans) => kmeans,
    };

    let clustering = kmeans.cluster(settings.max_iterations);

    // delete all the collections
    // NOTE: For some reason, if a collection has too many songs, it will fail to delete with "DbError(Db(Tx("Max transaction entries limit exceeded")))"
    // (this was happening with 892 songs in a collection)
    for collection in Collection::read_all(db).await? {
        Collection::delete(db, collection.id.clone()).await?;
    }

    // get the clusters from the clustering
    let clusters = extract_clusters(clustering);

    // create the collections
    for (i, cluster) in clusters.iter().filter(|c| !c.is_empty()).enumerate() {
        let collection = Collection::create(
            db,
            Collection {
                id: Collection::generate_id(),
                name: format!("Collection {i}").into(),
                runtime: Duration::default(),
                song_count: Default::default(),
            },
        )
        .await?
        .ok_or(Error::NotCreated)?;

        let mut songs = Vec::with_capacity(cluster.len());

        for analysis in cluster {
            songs.push(Analysis::read_song(db, analysis.id.clone()).await?.id);
        }

        Collection::add_songs(db, collection.id.clone(), &songs).await?;
    }

    info!("Library recluster complete");
    info!("Library brief: {:?}", brief(db).await?);

    Ok(())
}

/// Get a brief overview of the library.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
#[instrument]
pub async fn brief<C: Connection>(db: &Surreal<C>) -> Result<LibraryBrief, Error> {
    Ok(LibraryBrief {
        artists: count_artists(db).await?,
        albums: count_albums(db).await?,
        songs: count_songs(db).await?,
        playlists: count_playlists(db).await?,
        collections: count_collections(db).await?,
    })
}

/// Get the full library.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
#[instrument]
pub async fn full<C: Connection>(db: &Surreal<C>) -> Result<LibraryFull, Error> {
    Ok(LibraryFull {
        artists: Artist::read_all(db).await?.into(),
        albums: Album::read_all(db).await?.into(),
        songs: Song::read_all(db).await?.into(),
        playlists: Playlist::read_all(db).await?.into(),
        collections: Collection::read_all(db).await?.into(),
    })
}

/// Get the health of the library.
///
/// This function will return the health of the library, including the number of orphaned items.
///
/// # Errors
///
/// This function will return an error if there is an error reading from the database.
#[instrument]
pub async fn health<C: Connection>(db: &Surreal<C>) -> Result<LibraryHealth, Error> {
    Ok(LibraryHealth {
        artists: count_artists(db).await?,
        albums: count_albums(db).await?,
        songs: count_songs(db).await?,
        #[cfg(feature = "analysis")]
        unanalyzed_songs: Some(count_unanalyzed_songs(db).await?),
        #[cfg(not(feature = "analysis"))]
        unanalyzed_songs: None,
        playlists: count_playlists(db).await?,
        collections: count_collections(db).await?,
        orphaned_artists: count_orphaned_artists(db).await?,
        orphaned_albums: count_orphaned_albums(db).await?,
        orphaned_playlists: count_orphaned_playlists(db).await?,
        orphaned_collections: count_orphaned_collections(db).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init;

    use mecomp_storage::db::init_database;
    use mecomp_storage::db::schemas::song::{SongChangeSet, SongMetadata};
    use mecomp_storage::test_utils::{
        arb_song_case, arb_vec, create_song_metadata, create_song_with_overrides,
        init_test_database, SongCase, ARTIST_NAME_SEPARATOR,
    };
    use one_or_many::OneOrMany;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_rescan() {
        init();
        let tempdir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();

        // populate the tempdir with songs that aren't in the database
        let song_cases = arb_vec(&arb_song_case(), 10..=15)();
        let metadatas = song_cases
            .into_iter()
            .map(|song_case| create_song_metadata(&tempdir, song_case))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        // also make some songs that are in the database
        //  - a song that whose file was deleted
        let song_with_nonexistent_path = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                path: Some(tempdir.path().join("nonexistent.mp3")),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let mut metadata_of_song_with_outdated_metadata =
            create_song_metadata(&tempdir, arb_song_case()()).unwrap();
        metadata_of_song_with_outdated_metadata.genre = OneOrMany::None;
        let song_with_outdated_metadata =
            Song::try_load_into_db(&db, metadata_of_song_with_outdated_metadata)
                .await
                .unwrap();

        // rescan the library
        rescan(
            &db,
            &[tempdir.path().to_owned()],
            Some(ARTIST_NAME_SEPARATOR),
            Some(ARTIST_NAME_SEPARATOR),
            MetadataConflictResolution::Overwrite,
        )
        .await
        .unwrap();

        // check that everything was done correctly
        // - `song_with_nonexistent_path` was deleted
        assert_eq!(
            Song::read(&db, song_with_nonexistent_path.id)
                .await
                .unwrap(),
            None
        );
        // - `song_with_outdated_metadata` was updated
        assert!(Song::read(&db, song_with_outdated_metadata.id)
            .await
            .unwrap()
            .unwrap()
            .genre
            .is_some());
        // - all the other songs were added
        //   and their artists, albums, and album_artists were added and linked correctly
        for metadata in metadatas {
            // the song was created
            let song = Song::read_by_path(&db, metadata.path.clone())
                .await
                .unwrap();
            assert!(song.is_some());
            let song = song.unwrap();

            // the song's metadata is correct
            assert_eq!(SongMetadata::from(&song), metadata);

            // the song's artists were created
            let artists = Artist::read_by_names(&db, &Vec::from(metadata.artist.clone()))
                .await
                .unwrap();
            assert_eq!(artists.len(), metadata.artist.len());
            // the song is linked to the artists
            for artist in &artists {
                assert!(metadata.artist.contains(&artist.name));
                assert!(Artist::read_songs(&db, artist.id.clone())
                    .await
                    .unwrap()
                    .contains(&song));
            }
            // the artists are linked to the song
            if let Ok(song_artists) = Song::read_artist(&db, song.id.clone()).await {
                for artist in &artists {
                    assert!(song_artists.contains(&artist));
                }
            } else {
                panic!("Error reading song artists");
            }

            // the song's album was created
            let album = Album::read_by_name_and_album_artist(
                &db,
                &metadata.album,
                metadata.album_artist.clone(),
            )
            .await
            .unwrap();
            assert!(album.is_some());
            let album = album.unwrap();
            // the song is linked to the album
            assert_eq!(
                Song::read_album(&db, song.id.clone()).await.unwrap(),
                Some(album.clone())
            );
            // the album is linked to the song
            assert!(Album::read_songs(&db, album.id.clone())
                .await
                .unwrap()
                .contains(&song));

            // the album's album artists were created
            let album_artists =
                Artist::read_by_names(&db, &Vec::from(metadata.album_artist.clone()))
                    .await
                    .unwrap();
            assert_eq!(album_artists.len(), metadata.album_artist.len());
            // the album is linked to the album artists
            for album_artist in album_artists {
                assert!(metadata.album_artist.contains(&album_artist.name));
                assert!(Artist::read_albums(&db, album_artist.id.clone())
                    .await
                    .unwrap()
                    .contains(&album));
            }
        }
    }

    #[tokio::test]
    async fn test_analyze() {
        init();
        let dir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();

        // load some songs into the database
        let song_cases = arb_vec(&arb_song_case(), 10..=15)();
        let song_cases = song_cases.into_iter().enumerate().map(|(i, sc)| SongCase {
            song: i as u8,
            ..sc
        });
        let metadatas = song_cases
            .into_iter()
            .map(|song_case| create_song_metadata(&dir, song_case))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for metadata in &metadatas {
            Song::try_load_into_db(&db, metadata.clone()).await.unwrap();
        }

        // check that there are no analyses before.
        assert_eq!(
            Analysis::read_songs_without_analysis(&db)
                .await
                .unwrap()
                .len(),
            metadatas.len()
        );

        // analyze the library
        analyze(&db).await.unwrap();

        // check that all the songs have analyses
        assert_eq!(
            Analysis::read_songs_without_analysis(&db)
                .await
                .unwrap()
                .len(),
            0
        );
        for metadata in &metadatas {
            let song = Song::read_by_path(&db, metadata.path.clone())
                .await
                .unwrap()
                .unwrap();
            let analysis = Analysis::read_for_song(&db, song.id.clone()).await.unwrap();
            assert!(analysis.is_some());
        }

        // check that if we ask for the nearest neighbors of one of these songs, we get all the other songs
        for analysis in Analysis::read_all(&db).await.unwrap() {
            let neighbors = Analysis::nearest_neighbors(&db, analysis.id.clone(), 100)
                .await
                .unwrap();
            assert!(!neighbors.contains(&analysis));
            assert_eq!(neighbors.len(), metadatas.len() - 1);
            assert_eq!(
                neighbors.len(),
                neighbors
                    .iter()
                    .map(|n| n.id.clone())
                    .collect::<HashSet<_>>()
                    .len()
            );
        }
    }

    #[tokio::test]
    #[ignore = "uses the real filesystem and database"]
    async fn test_recluster() {
        init();
        mecomp_storage::db::set_database_path("/home/anthony/.local/share/mecomp/db".into())
        let settings = ReclusterSettings {
            gap_statistic_reference_datasets: 50,
            max_clusters: 16,
            max_iterations: 30,
        };
            .unwrap();
        let db = init_database().await.unwrap();

        // // load some songs into the database
        // let song_cases = arb_vec(&arb_song_case(), 10..=15)();
        // let metadatas = song_cases
        //     .into_iter()
        //     .map(|song_case| create_song_metadata(&tempfile::tempdir().unwrap(), song_case))
        //     .collect::<Result<Vec<_>, _>>()
        //     .unwrap();
        // for metadata in &metadatas {
        //     Song::try_load_into_db(&db, metadata.clone()).await.unwrap();
        // }

        // // analyze the library
        // analyze(&db).await.unwrap();

        // recluster the library
        recluster(&db, &settings).await.unwrap();

        // check that there are collections
        let collections = Collection::read_all(&db).await.unwrap();
        assert!(!collections.is_empty());
        for collection in collections {
            let songs = Collection::read_songs(&db, collection.id.clone())
                .await
                .unwrap();
            assert!(!songs.is_empty());
        }
    }

    #[tokio::test]
    async fn test_brief() {
        init();
        let db = init_test_database().await.unwrap();
        let brief = brief(&db).await.unwrap();
        assert_eq!(brief.artists, 0);
        assert_eq!(brief.albums, 0);
        assert_eq!(brief.songs, 0);
        assert_eq!(brief.playlists, 0);
        assert_eq!(brief.collections, 0);
    }

    #[tokio::test]
    async fn test_full() {
        init();
        let db = init_test_database().await.unwrap();
        let full = full(&db).await.unwrap();
        assert_eq!(full.artists.len(), 0);
        assert_eq!(full.albums.len(), 0);
        assert_eq!(full.songs.len(), 0);
        assert_eq!(full.playlists.len(), 0);
        assert_eq!(full.collections.len(), 0);
    }

    #[tokio::test]
    async fn test_health() {
        init();
        let db = init_test_database().await.unwrap();
        let health = health(&db).await.unwrap();
        assert_eq!(health.artists, 0);
        assert_eq!(health.albums, 0);
        assert_eq!(health.songs, 0);
        #[cfg(feature = "analysis")]
        assert_eq!(health.unanalyzed_songs, Some(0));
        #[cfg(not(feature = "analysis"))]
        assert_eq!(health.unanalyzed_songs, None);
        assert_eq!(health.playlists, 0);
        assert_eq!(health.collections, 0);
        assert_eq!(health.orphaned_artists, 0);
        assert_eq!(health.orphaned_albums, 0);
        assert_eq!(health.orphaned_playlists, 0);
        assert_eq!(health.orphaned_collections, 0);
    }
}
