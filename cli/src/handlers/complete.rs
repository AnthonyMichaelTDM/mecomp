//! This module provides functionality for generating completion candidates for
//! things like the import/export commands in the CLI.

use clap::builder::StyledStr;
use clap_complete::CompletionCandidate;
use mecomp_prost::MusicPlayerClient;

#[derive(Debug, PartialEq, Eq)]
pub enum CompletableTable {
    Artist,
    Album,
    Song,
    Playlist,
    DynamicPlaylist,
    Collection,
}

/// Generate completion candidates for items in the database,
/// Given the table name, returns a function that can be used to get completion candidates
/// from that table.
pub fn complete_things(table: CompletableTable) -> impl Fn() -> Vec<CompletionCandidate> {
    move || {
        // needs to be a multi-threaded runtime or else it will hang when trying to connect
        // to the daemon
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        let g = rt.enter();

        let handle = tokio::runtime::Handle::current();

        let mut client: MusicPlayerClient = match mecomp_prost::init_client(6600) {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to connect to daemon: {e}");
                return vec![];
            }
        };

        let candidates = match table {
            CompletableTable::Song => get_song_candidates(&handle, &mut client),
            CompletableTable::Album => get_album_candidates(&handle, &mut client),
            CompletableTable::Artist => get_artist_candidates(&handle, &mut client),
            CompletableTable::Playlist => get_playlist_candidates(&handle, &mut client),
            CompletableTable::DynamicPlaylist => {
                get_dynamic_playlist_candidates(&handle, &mut client)
            }
            CompletableTable::Collection => get_collection_candidates(&handle, &mut client),
        };
        drop(g); // Exit the Tokio runtime context

        candidates
            .into_iter()
            .map(|(id, help)| CompletionCandidate::new(id).help(Some(help)))
            .collect::<Vec<_>>()
    }
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
                song.id.id,
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
                album.id.id,
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
                artist.id.id,
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
                playlist.id.id,
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
                collection.id.id,
                StyledStr::from(format!("\"{}\"", collection.name)),
            )
        })
        .collect()
}
