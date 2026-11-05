//! This module provides functionality for generating completion candidates for
//! things like the import/export commands in the CLI.

use std::ffi::OsStr;

use clap::builder::StyledStr;
use clap_complete::CompletionCandidate;
use mecomp_prost::MusicPlayerClient;

#[derive(Debug, PartialEq, Eq)]
pub enum CompletableTable {
    Artist,
    Album,
    Song,
    Playlist,
    Dynamic,
    Collection,
}

/// The argument should be a fully-qualified record ID, e.g. "song:abcd-1234-efgh-5678".
///
/// The table is inferred from the prefix before the colon.
/// The completion candidates will include all valid *tables* if the prefix is
/// missing or incomplete, but if the prefix is valid then the candidates will be records from that table.
pub fn thing_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    let input = current.to_string_lossy();

    // Check if the input contains a colon to separate table and ID
    if let Some(colon_pos) = input.find(':') {
        let table_str = &input[..colon_pos];
        let id_prefix = &input[colon_pos + 1..];
        let id_prefix = id_prefix.to_ascii_uppercase();

        // Determine the table
        let table = match table_str {
            "song" => CompletableTable::Song,
            "album" => CompletableTable::Album,
            "artist" => CompletableTable::Artist,
            "playlist" => CompletableTable::Playlist,
            "dynamic" => CompletableTable::Dynamic,
            "collection" => CompletableTable::Collection,
            _ => return vec![], // Unknown table, return no candidates
        };

        // Get candidates from the specified table
        let candidates_fn = thing_candidates(table);
        let candidates = candidates_fn();

        // Filter candidates based on the ID prefix
        candidates
            .into_iter()
            .filter_map(|candidate| {
                let id = candidate.get_value().to_string_lossy().to_ascii_uppercase();
                id.starts_with(&id_prefix).then(|| {
                    CompletionCandidate::new(format!("{table_str}:{id}"))
                        .help(candidate.get_help().cloned())
                })
            })
            .collect()
    } else {
        // No colon found, suggest table names
        get_tables()
            .into_iter()
            .map(|(name, help)| CompletionCandidate::new(format!("{name}:")).help(Some(help)))
            .collect()
    }
}

/// Generate completion candidates for items in the database,
/// Given the table name, returns a function that can be used to get completion candidates
/// from that table.
pub fn thing_candidates(table: CompletableTable) -> impl Fn() -> Vec<CompletionCandidate> {
    move || {
        // needs to be a multi-threaded runtime or else it will hang when trying to connect
        // to the daemon
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        let g = rt.enter();

        let mut client: MusicPlayerClient = mecomp_prost::lazy_init_client(6600);

        let candidates = match table {
            CompletableTable::Song => rt.block_on(get_song_candidates(&mut client)),
            CompletableTable::Album => rt.block_on(get_album_candidates(&mut client)),
            CompletableTable::Artist => rt.block_on(get_artist_candidates(&mut client)),
            CompletableTable::Playlist => rt.block_on(get_playlist_candidates(&mut client)),
            CompletableTable::Dynamic => rt.block_on(get_dynamic_playlist_candidates(&mut client)),
            CompletableTable::Collection => rt.block_on(get_collection_candidates(&mut client)),
        };
        drop(g); // Exit the Tokio runtime context

        candidates
            .into_iter()
            .map(|(id, help)| CompletionCandidate::new(id).help(Some(help)))
            .collect::<Vec<_>>()
    }
}

fn get_tables() -> Vec<(String, StyledStr)> {
    vec![
        ("song".to_string(), StyledStr::from("A song")),
        ("album".to_string(), StyledStr::from("An album")),
        ("artist".to_string(), StyledStr::from("An artist")),
        ("playlist".to_string(), StyledStr::from("A playlist")),
        (
            "dynamic_playlist".to_string(),
            StyledStr::from("A dynamic playlist"),
        ),
        ("collection".to_string(), StyledStr::from("A collection")),
    ]
}

async fn get_song_candidates(client: &mut MusicPlayerClient) -> Vec<(String, StyledStr)> {
    let response = client.library_songs(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch songs: {e}");
        return vec![];
    }
    let songs = response.unwrap().into_inner().songs;

    songs
        .into_iter()
        .map(|song| {
            (
                song.id.id,
                StyledStr::from(format!(
                    "\"{}\" (by: {:?}, album: {})",
                    song.title, song.artists, song.album
                )),
            )
        })
        .collect()
}

async fn get_album_candidates(client: &mut MusicPlayerClient) -> Vec<(String, StyledStr)> {
    let response = client.library_albums(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch albums: {e}");
        return vec![];
    }
    let albums = response.unwrap().into_inner().albums;

    albums
        .into_iter()
        .map(|album| {
            (
                album.id.id,
                StyledStr::from(format!("\"{}\" (by: {:?})", album.title, album.artists)),
            )
        })
        .collect()
}

async fn get_artist_candidates(client: &mut MusicPlayerClient) -> Vec<(String, StyledStr)> {
    let response = client.library_artists(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch artists: {e}");
        return vec![];
    }
    let artists = response.unwrap().into_inner().artists;

    artists
        .into_iter()
        .map(|artist| {
            (
                artist.id.id,
                StyledStr::from(format!("\"{}\"", artist.name)),
            )
        })
        .collect()
}

async fn get_playlist_candidates(client: &mut MusicPlayerClient) -> Vec<(String, StyledStr)> {
    let response = client.library_playlists(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch playlists: {e}");
        return vec![];
    }
    let playlists = response.unwrap().into_inner().playlists;

    playlists
        .into_iter()
        .map(|playlist| {
            (
                playlist.id.id,
                StyledStr::from(format!("\"{}\"", playlist.name)),
            )
        })
        .collect()
}

async fn get_dynamic_playlist_candidates(
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = client.library_dynamic_playlists(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch dynamic playlists: {e}");
        return vec![];
    }
    let playlists = response.unwrap().into_inner().playlists;

    playlists
        .into_iter()
        .map(|playlist| {
            (
                playlist.id.id,
                StyledStr::from(format!("\"{}\" ({})", playlist.name, playlist.query)),
            )
        })
        .collect()
}

async fn get_collection_candidates(client: &mut MusicPlayerClient) -> Vec<(String, StyledStr)> {
    let response = client.library_collections(()).await;
    if let Err(e) = response {
        eprintln!("Failed to fetch collections: {e}");
        return vec![];
    }
    let collections = response.unwrap().into_inner().collections;

    collections
        .into_iter()
        .map(|collection| {
            (
                collection.id.id,
                StyledStr::from(format!("\"{}\"", collection.name)),
            )
        })
        .collect()
}
