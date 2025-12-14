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
            .filter(|candidate| {
                candidate
                    .get_value()
                    .to_string_lossy()
                    .starts_with(id_prefix)
            })
            .collect()
    } else {
        // No colon found, suggest table names
        get_tables()
            .into_iter()
            .map(|(name, help)| CompletionCandidate::new(name).help(Some(help)))
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
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        let g = rt.enter();

        let handle = tokio::runtime::Handle::current();

        let mut client: MusicPlayerClient = mecomp_prost::lazy_init_client(6600);

        let candidates = match table {
            CompletableTable::Song => get_song_candidates(&handle, &mut client),
            CompletableTable::Album => get_album_candidates(&handle, &mut client),
            CompletableTable::Artist => get_artist_candidates(&handle, &mut client),
            CompletableTable::Playlist => get_playlist_candidates(&handle, &mut client),
            CompletableTable::Dynamic => get_dynamic_playlist_candidates(&handle, &mut client),
            CompletableTable::Collection => get_collection_candidates(&handle, &mut client),
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

fn get_song_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_songs(()));
    if let Err(e) = response {
        eprintln!("Failed to fetch songs: {e}");
        return vec![];
    }
    let songs = response.unwrap().into_inner().songs;

    songs
        .into_iter()
        .map(|song| {
            (
                song.id.to_string(),
                StyledStr::from(format!(
                    "\"{}\" (by: {:?}, album: {})",
                    song.title, song.artists, song.album
                )),
            )
        })
        .collect()
}

fn get_album_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_albums(()));
    if let Err(e) = response {
        eprintln!("Failed to fetch albums: {e}");
        return vec![];
    }
    let albums = response.unwrap().into_inner().albums;

    albums
        .into_iter()
        .map(|album| {
            (
                album.id.to_string(),
                StyledStr::from(format!("\"{}\" (by: {:?})", album.title, album.artists)),
            )
        })
        .collect()
}

fn get_artist_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_artists(()));
    if let Err(e) = response {
        eprintln!("Failed to fetch artists: {e}");
        return vec![];
    }
    let artists = response.unwrap().into_inner().artists;

    artists
        .into_iter()
        .map(|artist| {
            (
                artist.id.to_string(),
                StyledStr::from(format!("\"{}\"", artist.name)),
            )
        })
        .collect()
}

fn get_playlist_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_playlists(()));
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

fn get_dynamic_playlist_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_dynamic_playlists(()));
    if let Err(e) = response {
        eprintln!("Failed to fetch dynamic playlists: {e}");
        return vec![];
    }
    let playlists = response.unwrap().into_inner().playlists;

    playlists
        .into_iter()
        .map(|playlist| {
            (
                playlist.id.to_string(),
                StyledStr::from(format!("\"{}\" ({})", playlist.name, playlist.query)),
            )
        })
        .collect()
}

fn get_collection_candidates(
    rt: &tokio::runtime::Handle,
    client: &mut MusicPlayerClient,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_collections(()));
    if let Err(e) = response {
        eprintln!("Failed to fetch collections: {e}");
        return vec![];
    }
    let collections = response.unwrap().into_inner().collections;

    collections
        .into_iter()
        .map(|collection| {
            (
                collection.id.to_string(),
                StyledStr::from(format!("\"{}\"", collection.name)),
            )
        })
        .collect()
}
