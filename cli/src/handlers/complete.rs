//! This module provides functionality for generating completion candidates for
//! things like the import/export commands in the CLI.

use clap::builder::StyledStr;
use clap_complete::CompletionCandidate;
use mecomp_core::rpc::MusicPlayerClient;
use mecomp_storage::db::schemas::dynamic::query::Compile;

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

        let client: MusicPlayerClient = match handle.block_on(mecomp_core::rpc::init_client(6600)) {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to connect to daemon: {e}");
                return vec![];
            }
        };
        let ctx = tarpc::context::current();

        let candidates = match table {
            CompletableTable::Song => get_song_candidates(&handle, &client, ctx),
            CompletableTable::Album => get_album_candidates(&handle, &client, ctx),
            CompletableTable::Artist => get_artist_candidates(&handle, &client, ctx),
            CompletableTable::Playlist => get_playlist_candidates(&handle, &client, ctx),
            CompletableTable::DynamicPlaylist => {
                get_dynamic_playlist_candidates(&handle, &client, ctx)
            }
            CompletableTable::Collection => get_collection_candidates(&handle, &client, ctx),
        };
        drop(g); // Exit the Tokio runtime context

        candidates
            .iter()
            .cloned()
            .map(|(id, help)| CompletionCandidate::new(id).help(Some(help)))
            .collect::<Vec<_>>()
    }
}

fn get_song_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_songs_brief(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch songs: {e}");
        return vec![];
    }
    let songs = response.unwrap().unwrap_or_default();

    songs
        .into_iter()
        .map(|song| {
            (
                song.id.key().to_string(),
                StyledStr::from(format!(
                    "\"{}\" (by: {:?}, album: {})",
                    song.title, song.artist, song.album
                )),
            )
        })
        .collect()
}

fn get_album_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_albums_brief(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch albums: {e}");
        return vec![];
    }
    let albums = response.unwrap().unwrap_or_default();

    albums
        .into_iter()
        .map(|album| {
            (
                album.id.key().to_string(),
                StyledStr::from(format!("\"{}\" (by: {:?})", album.title, album.artist)),
            )
        })
        .collect()
}

fn get_artist_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_artists_brief(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch artists: {e}");
        return vec![];
    }
    let artists = response.unwrap().unwrap_or_default();

    artists
        .into_iter()
        .map(|artist| {
            (
                artist.id.key().to_string(),
                StyledStr::from(format!("\"{}\"", artist.name)),
            )
        })
        .collect()
}

fn get_playlist_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_playlists_brief(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch playlists: {e}");
        return vec![];
    }
    let playlists = response.unwrap().unwrap_or_default();

    playlists
        .into_iter()
        .map(|playlist| {
            (
                playlist.id.key().to_string(),
                StyledStr::from(format!("\"{}\"", playlist.name)),
            )
        })
        .collect()
}

fn get_dynamic_playlist_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.dynamic_playlist_list(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch dynamic playlists: {e}");
        return vec![];
    }
    let playlists = response.unwrap();

    playlists
        .into_iter()
        .map(|playlist| {
            (
                playlist.id.key().to_string(),
                StyledStr::from(format!(
                    "\"{}\" ({})",
                    playlist.name,
                    playlist.query.compile_for_storage()
                )),
            )
        })
        .collect()
}

fn get_collection_candidates(
    rt: &tokio::runtime::Handle,
    client: &MusicPlayerClient,
    ctx: tarpc::context::Context,
) -> Vec<(String, StyledStr)> {
    let response = rt.block_on(client.library_collections_brief(ctx));
    if let Err(e) = response {
        eprintln!("Failed to fetch collections: {e}");
        return vec![];
    }
    let collections = response.unwrap().unwrap_or_default();

    collections
        .into_iter()
        .map(|collection| {
            (
                collection.id.key().to_string(),
                StyledStr::from(format!("\"{}\"", collection.name)),
            )
        })
        .collect()
}
