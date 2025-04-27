use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use log::{debug, error, info, warn};
use mecomp_analysis::{
    clustering::{ClusteringHelper, KOptimal, NotInitialized},
    decoder::{Decoder, MecompDecoder},
};
use mecomp_core::{
    config::ReclusterSettings,
    state::library::{LibraryBrief, LibraryFull, LibraryHealth},
};
use one_or_many::OneOrMany;
use surrealdb::{Connection, Surreal};
use tap::TapFallible;
use tracing::{Instrument, instrument};
use walkdir::WalkDir;

use mecomp_storage::{
    db::{
        health::{
            count_albums, count_artists, count_collections, count_dynamic_playlists,
            count_orphaned_albums, count_orphaned_artists, count_orphaned_collections,
            count_orphaned_playlists, count_playlists, count_songs, count_unanalyzed_songs,
        },
        schemas::{
            album::Album,
            analysis::Analysis,
            artist::Artist,
            collection::Collection,
            dynamic::DynamicPlaylist,
            playlist::Playlist,
            song::{Song, SongMetadata},
        },
    },
    errors::Error,
    util::MetadataConflictResolution,
};

use crate::termination::InterruptReceiver;

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
    artist_name_separator: &OneOrMany<String>,
    protected_artist_names: &OneOrMany<String>,
    genre_separator: Option<&str>,
    conflict_resolution_mode: MetadataConflictResolution,
) -> Result<(), Error> {
    // get all the songs in the current library
    let songs = Song::read_all(db).await?;
    let mut paths_to_skip = HashSet::new(); // use a hashset because hashing is faster than linear search, especially for large libraries

    // for each song, check if the file still exists
    async {
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
            match SongMetadata::load_from_path(path.clone(), artist_name_separator,protected_artist_names, genre_separator) {
                // if we have metadata and the metadata is different from the song's metadata, and ...
                Ok(metadata) if metadata != SongMetadata::from(&song) => {
                    let log_postfix = if conflict_resolution_mode == MetadataConflictResolution::Skip {
                        "but conflict resolution mode is \"skip\", so we do nothing"
                    } else {
                        "resolving conflict"
                    };
                    info!(
                        "{} has conflicting metadata with index, {log_postfix}",
                        path.to_string_lossy(),
                    );

                    match conflict_resolution_mode {
                        // ... we are in "overwrite" mode, update the song's metadata
                        MetadataConflictResolution::Overwrite => {
                            // if the file has been modified, update the song's metadata
                            Song::update(db, song.id.clone(), metadata.merge_with_song(&song)).await?;
                        }
                        // ... we are in "skip" mode, do nothing
                        MetadataConflictResolution::Skip => {
                            continue;
                        }
                    }
                }
                // if we have an error, delete the song from the library
                Err(e) => {
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

        <Result<(), Error>>::Ok(())
    }.instrument(tracing::info_span!("Checking library for missing or outdated songs")).await?;

    // now, index all the songs in the library that haven't been indexed yet
    let mut visited_paths = paths_to_skip;

    debug!("Indexing paths: {paths:?}");
    async {
        for path in paths
            .iter()
            .filter_map(|p| {
                p.canonicalize()
                    .tap_err(|e| warn!("Error canonicalizing path: {e}"))
                    .ok()
            })
            .flat_map(|x| WalkDir::new(x).into_iter())
            .filter_map(|x| x.tap_err(|e| warn!("Error reading path: {e}")).ok())
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
                protected_artist_names,
                genre_separator,
            ) {
                Ok(metadata) => Song::try_load_into_db(db, metadata).await.map_or_else(
                    |e| warn!("Error indexing {}: {}", path.path().to_string_lossy(), e),
                    |_| debug!("Indexed {}", path.path().to_string_lossy()),
                ),
                Err(e) => warn!(
                    "Error reading metadata for {}: {}",
                    path.path().to_string_lossy(),
                    e
                ),
            }
        }

        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Indexing new songs"))
    .await?;

    // find and delete any remaining orphaned albums and artists
    // TODO: create a custom query for this

    async {
        for album in Album::read_all(db).await? {
            if Album::repair(db, album.id.clone()).await? {
                info!("Deleted orphaned album {}", album.id.clone());
                Album::delete(db, album.id.clone()).await?;
            }
        }
        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Repairing albums"))
    .await?;
    async {
        for artist in Artist::read_all(db).await? {
            if Artist::repair(db, artist.id.clone()).await? {
                info!("Deleted orphaned artist {}", artist.id.clone());
                Artist::delete(db, artist.id.clone()).await?;
            }
        }
        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Repairing artists"))
    .await?;
    async {
        for collection in Collection::read_all(db).await? {
            if Collection::repair(db, collection.id.clone()).await? {
                info!("Deleted orphaned collection {}", collection.id.clone());
                Collection::delete(db, collection.id.clone()).await?;
            }
        }
        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Repairing collections"))
    .await?;

    let orphans = Playlist::delete_orphaned(db)
        .instrument(tracing::info_span!("Repairing playlists"))
        .await?;
    if !orphans.is_empty() {
        info!("Deleted orphaned playlists: {orphans:?}");
    }

    info!("Library rescan complete");
    info!("Library brief: {:?}", brief(db).await?);

    Ok(())
}

/// Analyze the library.
///
/// In order, this function will:
/// - if `overwrite` is true, delete all existing analyses.
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
pub async fn analyze<C: Connection>(
    db: &Surreal<C>,
    mut interrupt: InterruptReceiver,
    overwrite: bool,
) -> Result<(), Error> {
    if overwrite {
        // delete all the analyses
        async {
            for analysis in Analysis::read_all(db).await? {
                Analysis::delete(db, analysis.id.clone()).await?;
            }
            <Result<(), Error>>::Ok(())
        }
        .instrument(tracing::info_span!("Deleting existing analyses"))
        .await?;
    }

    // get all the songs that don't have an analysis
    let songs_to_analyze: Vec<Song> = Analysis::read_songs_without_analysis(db).await?;
    // crate a hashmap mapping paths to song ids
    let paths = songs_to_analyze
        .iter()
        .map(|song| (song.path.clone(), song.id.clone()))
        .collect::<HashMap<_, _>>();

    let keys = paths.keys().cloned().collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel();

    let Ok(decoder) = MecompDecoder::new() else {
        error!("Error creating decoder");
        return Ok(());
    };

    // analyze the songs in batches, this is a blocking operation
    let handle = tokio::task::spawn_blocking(move || decoder.analyze_paths_with_callback(keys, tx));
    let abort = handle.abort_handle();

    async {
        for (song_path, maybe_analysis) in rx {
            if interrupt.is_stopped() {
                info!("Analysis interrupted");
                break;
            }

            let Some(song_id) = paths.get(&song_path) else {
                error!("No song id found for path: {}", song_path.to_string_lossy());
                continue;
            };

            match maybe_analysis {
                Ok(analysis) => Analysis::create(
                    db,
                    song_id.clone(),
                    Analysis {
                        id: Analysis::generate_id(),
                        features: *analysis.inner(),
                    },
                )
                .await?
                .map_or_else(
                    || {
                        warn!(
                        "Error analyzing {}: song either wasn't found or already has an analysis",
                        song_path.to_string_lossy()
                    );
                    },
                    |_| debug!("Analyzed {}", song_path.to_string_lossy()),
                ),
                Err(e) => {
                    error!("Error analyzing {}: {}", song_path.to_string_lossy(), e);
                }
            }
        }

        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Adding analyses to database"))
    .await?;

    tokio::select! {
        // wait for the interrupt signal
        _ = interrupt.wait() => {
            info!("Analysis interrupted");
            abort.abort();
        }
        // wait for the analysis to finish
        result = handle => match result {
            Ok(Ok(())) => {
                info!("Analysis complete");
                info!("Library brief: {:?}", brief(db).await?);
            }
            Ok(Err(e)) => {
                error!("Error analyzing songs: {e}");
            }
            Err(e) => {
                error!("Error joining task: {e}");
            }
        }
    }

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
    settings: ReclusterSettings,
    mut interrupt: InterruptReceiver,
) -> Result<(), Error> {
    // collect all the analyses
    let samples = Analysis::read_all(db).await?;

    if samples.is_empty() {
        info!("No analyses found, nothing to recluster");
        return Ok(());
    }

    let samples_ref = samples.clone();

    // use clustering algorithm to cluster the analyses
    let clustering = move || {
        let model: ClusteringHelper<NotInitialized> = match ClusteringHelper::new(
            samples_ref
                .iter()
                .map(Into::into)
                .collect::<Vec<mecomp_analysis::Analysis>>()
                .into(),
            settings.max_clusters,
            KOptimal::GapStatistic {
                b: settings.gap_statistic_reference_datasets,
            },
            settings.algorithm.into(),
            settings.projection_method.into(),
        ) {
            Err(e) => {
                error!("There was an error creating the clustering helper: {e}",);
                return None;
            }
            Ok(kmeans) => kmeans,
        };

        let model = match model.initialize() {
            Err(e) => {
                error!("There was an error initializing the clustering helper: {e}",);
                return None;
            }
            Ok(kmeans) => kmeans.cluster(),
        };

        Some(model)
    };

    // use clustering algorithm to cluster the analyses
    let handle = tokio::task::spawn_blocking(clustering)
        .instrument(tracing::info_span!("Clustering library"));
    let abort = handle.inner().abort_handle();

    // wait for the clustering to finish
    let model = tokio::select! {
        _ = interrupt.wait() => {
            info!("Reclustering interrupted");
            abort.abort();
            return Ok(());
        }
        result = handle => match result {
            Ok(Some(model)) => model,
            Ok(None) => {
                return Ok(());
            }
            Err(e) => {
                error!("Error joining task: {e}");
                return Ok(());
            }
        }
    };

    // delete all the collections
    async {
        // NOTE: For some reason, if a collection has too many songs, it will fail to delete with "DbError(Db(Tx("Max transaction entries limit exceeded")))"
        // (this was happening with 892 songs in a collection)
        for collection in Collection::read_all(db).await? {
            Collection::delete(db, collection.id.clone()).await?;
        }

        <Result<(), Error>>::Ok(())
    }
    .instrument(tracing::info_span!("Deleting old collections"))
    .await?;

    // get the clusters from the clustering
    async {
        let clusters = model.extract_analysis_clusters(samples);

        // create the collections
        for (i, cluster) in clusters.iter().filter(|c| !c.is_empty()).enumerate() {
            let collection = Collection::create(
                db,
                Collection {
                    id: Collection::generate_id(),
                    name: format!("Collection {i}"),
                    runtime: Duration::default(),
                    song_count: Default::default(),
                },
            )
            .await?
            .ok_or(Error::NotCreated)?;

            let mut songs = Vec::with_capacity(cluster.len());

            async {
                for analysis in cluster {
                    songs.push(Analysis::read_song(db, analysis.id.clone()).await?.id);
                }

                Collection::add_songs(db, collection.id.clone(), songs).await?;

                <Result<(), Error>>::Ok(())
            }
            .instrument(tracing::info_span!("Adding songs to collection"))
            .await?;
        }
        Ok::<(), Error>(())
    }
    .instrument(tracing::info_span!("Creating new collections"))
    .await?;

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
        dynamic_playlists: count_dynamic_playlists(db).await?,
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
        dynamic_playlists: DynamicPlaylist::read_all(db).await?.into(),
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
        dynamic_playlists: count_dynamic_playlists(db).await?,
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

    use mecomp_core::config::{ClusterAlgorithm, ProjectionMethod};
    use mecomp_storage::db::schemas::song::{SongChangeSet, SongMetadata};
    use mecomp_storage::test_utils::{
        ARTIST_NAME_SEPARATOR, SongCase, arb_analysis_features, arb_song_case, arb_vec,
        create_song_metadata, create_song_with_overrides, init_test_database,
    };
    use one_or_many::OneOrMany;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
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
        // also add a "song" that can't be read
        let invalid_song_path = tempdir.path().join("invalid1.mp3");
        std::fs::write(&invalid_song_path, "this is not a song").unwrap();
        // add another invalid song, this time also put it in the database
        let invalid_song_path = tempdir.path().join("invalid2.mp3");
        std::fs::write(&invalid_song_path, "this is not a song").unwrap();
        let song_with_invalid_metadata = create_song_with_overrides(
            &db,
            arb_song_case()(),
            SongChangeSet {
                path: Some(tempdir.path().join("invalid2.mp3")),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // rescan the library
        rescan(
            &db,
            &[tempdir.path().to_owned()],
            &OneOrMany::One(ARTIST_NAME_SEPARATOR.to_string()),
            &OneOrMany::None,
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
        // - `song_with_invalid_metadata` was deleted
        assert_eq!(
            Song::read(&db, song_with_invalid_metadata.id)
                .await
                .unwrap(),
            None
        );
        // - `song_with_outdated_metadata` was updated
        assert!(
            Song::read(&db, song_with_outdated_metadata.id)
                .await
                .unwrap()
                .unwrap()
                .genre
                .is_some()
        );
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
            let artists = Artist::read_by_names(&db, Vec::from(metadata.artist.clone()))
                .await
                .unwrap();
            assert_eq!(artists.len(), metadata.artist.len());
            // the song is linked to the artists
            for artist in &artists {
                assert!(metadata.artist.contains(&artist.name));
                assert!(
                    Artist::read_songs(&db, artist.id.clone())
                        .await
                        .unwrap()
                        .contains(&song)
                );
            }
            // the artists are linked to the song
            if let Ok(song_artists) = Song::read_artist(&db, song.id.clone()).await {
                for artist in &artists {
                    assert!(song_artists.contains(artist));
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
            assert!(
                Album::read_songs(&db, album.id.clone())
                    .await
                    .unwrap()
                    .contains(&song)
            );

            // the album's album artists were created
            let album_artists =
                Artist::read_by_names(&db, Vec::from(metadata.album_artist.clone()))
                    .await
                    .unwrap();
            assert_eq!(album_artists.len(), metadata.album_artist.len());
            // the album is linked to the album artists
            for album_artist in album_artists {
                assert!(metadata.album_artist.contains(&album_artist.name));
                assert!(
                    Artist::read_albums(&db, album_artist.id.clone())
                        .await
                        .unwrap()
                        .contains(&album)
                );
            }
        }
    }

    #[tokio::test]
    async fn rescan_deletes_preexisting_orphans() {
        init();
        let tempdir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();

        // create a song with an artist and an album
        let metadata = create_song_metadata(&tempdir, arb_song_case()()).unwrap();
        let song = Song::try_load_into_db(&db, metadata.clone()).await.unwrap();

        // delete the song, leaving orphaned artist and album
        std::fs::remove_file(&song.path).unwrap();
        Song::delete(&db, (song.id.clone(), false)).await.unwrap();

        // rescan the library
        rescan(
            &db,
            &[tempdir.path().to_owned()],
            &OneOrMany::One(ARTIST_NAME_SEPARATOR.to_string()),
            &OneOrMany::None,
            Some(ARTIST_NAME_SEPARATOR),
            MetadataConflictResolution::Overwrite,
        )
        .await
        .unwrap();

        // check that the album and artist deleted
        assert_eq!(Song::read_all(&db).await.unwrap().len(), 0);
        assert_eq!(Album::read_all(&db).await.unwrap().len(), 0);
        assert_eq!(Artist::read_all(&db).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn rescan_deletes_orphaned_albums_and_artists() {
        init();
        let tempdir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();

        // create a song with an artist and an album
        let metadata = create_song_metadata(&tempdir, arb_song_case()()).unwrap();
        let song = Song::try_load_into_db(&db, metadata.clone()).await.unwrap();
        let artist = Artist::read_by_names(&db, Vec::from(metadata.artist.clone()))
            .await
            .unwrap()
            .pop()
            .unwrap();
        let album = Album::read_by_name_and_album_artist(
            &db,
            &metadata.album,
            metadata.album_artist.clone(),
        )
        .await
        .unwrap()
        .unwrap();

        // delete the song, leaving orphaned artist and album
        std::fs::remove_file(&song.path).unwrap();

        // rescan the library
        rescan(
            &db,
            &[tempdir.path().to_owned()],
            &OneOrMany::One(ARTIST_NAME_SEPARATOR.to_string()),
            &OneOrMany::None,
            Some(ARTIST_NAME_SEPARATOR),
            MetadataConflictResolution::Overwrite,
        )
        .await
        .unwrap();

        // check that the artist and album were deleted
        assert_eq!(Artist::read(&db, artist.id.clone()).await.unwrap(), None);
        assert_eq!(Album::read(&db, album.id.clone()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_analyze() {
        init();
        let dir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();
        let interrupt = InterruptReceiver::dummy();

        // load some songs into the database
        let song_cases = arb_vec(&arb_song_case(), 10..=15)();
        let song_cases = song_cases.into_iter().enumerate().map(|(i, sc)| SongCase {
            song: u8::try_from(i).unwrap(),
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
        analyze(&db, interrupt, true).await.unwrap();

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

    #[rstest]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_recluster(
        #[values(ProjectionMethod::TSne, ProjectionMethod::None, ProjectionMethod::Pca)]
        projection_method: ProjectionMethod,
    ) {
        init();
        let dir = tempfile::tempdir().unwrap();
        let db = init_test_database().await.unwrap();
        let settings = ReclusterSettings {
            gap_statistic_reference_datasets: 5,
            max_clusters: 18,
            algorithm: ClusterAlgorithm::GMM,
            projection_method,
        };

        // load some songs into the database
        let song_cases = arb_vec(&arb_song_case(), 32..=32)();
        let song_cases = song_cases.into_iter().enumerate().map(|(i, sc)| SongCase {
            song: u8::try_from(i).unwrap(),
            ..sc
        });
        let metadatas = song_cases
            .into_iter()
            .map(|song_case| create_song_metadata(&dir, song_case))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut songs = Vec::with_capacity(metadatas.len());
        for metadata in &metadatas {
            songs.push(Song::try_load_into_db(&db, metadata.clone()).await.unwrap());
        }

        // load some dummy analyses into the database
        for song in &songs {
            Analysis::create(
                &db,
                song.id.clone(),
                Analysis {
                    id: Analysis::generate_id(),
                    features: arb_analysis_features()(),
                },
            )
            .await
            .unwrap();
        }

        // recluster the library
        recluster(&db, settings, InterruptReceiver::dummy())
            .await
            .unwrap();

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
