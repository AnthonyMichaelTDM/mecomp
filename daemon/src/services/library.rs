use std::{collections::HashSet, path::PathBuf};

use log::{debug, info, warn};
use mecomp_core::state::library::{LibraryBrief, LibraryFull, LibraryHealth};
use surrealdb::{Connection, Surreal};
use tap::TapFallible;
use tracing::instrument;
// use tokio::runtime::Handle;
use walkdir::WalkDir;

use mecomp_storage::{
    db::{
        health::{
            count_albums, count_artists, count_collections, count_orphaned_albums,
            count_orphaned_artists, count_orphaned_collections, count_orphaned_playlists,
            count_playlists, count_songs,
        },
        schemas::{
            album::Album,
            artist::Artist,
            collection::Collection,
            playlist::Playlist,
            song::{Song, SongMetadata},
        },
    },
    errors::Error,
    util::MetadataConflictResolution,
};

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
                if !metadata.is_same_song(&SongMetadata::from(&song)) =>
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
                if !metadata.is_same_song(&SongMetadata::from(&song)) =>
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
    let mut visited_paths = HashSet::new();
    visited_paths.extend(paths_to_skip);

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
        playlists: count_playlists(db).await?,
        collections: count_collections(db).await?,
        orphaned_artists: count_orphaned_artists(db).await?,
        orphaned_albums: count_orphaned_albums(db).await?,
        orphaned_playlists: count_orphaned_playlists(db).await?,
        orphaned_collections: count_orphaned_collections(db).await?,
    })
}
