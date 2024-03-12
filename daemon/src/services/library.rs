use std::{collections::HashSet, path::PathBuf};

use log::{debug, info, warn};
use mecomp_core::library::LibraryBrief;
use tap::TapFallible;
// use tokio::runtime::Handle;
use walkdir::WalkDir;

use mecomp_storage::{
    db::schemas::{
        album::Album,
        artist::Artist,
        playlist::Playlist,
        song::{Song, SongMetadata},
    },
    errors::Error,
    util::MetadataConflictResolution,
};

pub async fn rescan(
    paths: &[PathBuf],
    artist_name_separator: Option<&str>,
    genre_separator: Option<&'static str>,
    conflict_resolution_mode: MetadataConflictResolution,
) -> Result<(), Error> {
    info!("Rescanning library");
    // get all the songs in the current library
    let songs = Song::read_all().await?;
    let mut paths_to_skip = HashSet::new(); // use a hashset because hashing is faster than linear search, especially for large libraries

    // for each song, check if the file still exists
    for song in songs {
        let path = song.path.clone();
        if !path.exists() {
            // remove the song from the library
            warn!("Song {} no longer exists, deleting", path.to_string_lossy());
            Song::delete(song.id).await?;
        } else {
            // check if the metadata of the file is the same as the metadata in the database
            match SongMetadata::load_from_path(path.clone(), artist_name_separator, genre_separator)
            {
                Ok(metadata) => {
                    debug!("loaded metadata for {}", path.to_string_lossy());
                    if !metadata.is_same_song(&SongMetadata::from(&song)) {
                        let new_metadata = match conflict_resolution_mode {
                            MetadataConflictResolution::Merge => {
                                SongMetadata::merge(&SongMetadata::from(&song), &metadata)
                            }
                            MetadataConflictResolution::Overwrite => metadata,
                            MetadataConflictResolution::Skip => {
                                warn!(
                                    "{} has conflicting metadata with index, but conflict resolution mode is \"skip\", so we do nothing",
                                    path.to_string_lossy()
                                );
                                continue;
                            }
                        };
                        // if the file has been modified, update the song's metadata
                        Song::update_and_repair(
                            song.id.clone(),
                            new_metadata.merge_with_song(&song),
                        )
                        .await?;
                    }
                }
                Err(e) => {
                    warn!(
                        "Error reading metadata for {}: {}",
                        path.to_string_lossy(),
                        e
                    );
                    info!("assuming the file isn't a song or doesn't exist anymore, removing from library");
                    Song::delete(song.id).await?;
                }
            }
            // now, add the path to the list of paths to skip so that we don't index the song again
            paths_to_skip.insert(path);
        }
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
        .filter_map(|x| {
            if x.file_type().is_file() {
                Some(x)
            } else {
                None
            }
        })
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
            Ok(metadata) => match Song::try_load_into_db(metadata).await {
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
    info!("Library brief: {:?}", brief().await?);

    Ok(())
}

pub async fn brief() -> Result<LibraryBrief, Error> {
    Ok(LibraryBrief {
        artists: Artist::read_all().await?.len(),
        albums: Album::read_all().await?.len(),
        songs: Song::read_all().await?.len(),
        playlists: Playlist::read_all().await?.len(),
    })
}
