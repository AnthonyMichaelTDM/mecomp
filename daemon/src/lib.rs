//----------------------------------------------------------------------------------------- std lib
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
//--------------------------------------------------------------------------------- other libraries
use futures::{future, prelude::*};
use log::info;
use surrealdb::{engine::local::Db, Surreal};
use tarpc::{
    self,
    server::{incoming::Incoming as _, BaseChannel, Channel as _},
    tokio_serde::formats::Json,
};
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::AudioKernelSender,
    is_server_running,
    logger::{init_logger, init_tracing},
    rpc::{MusicPlayer as _, MusicPlayerClient},
};
use mecomp_storage::db::{init_database, set_database_path};

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
}

pub mod config;
pub mod controller;
#[cfg(feature = "dynamic_updates")]
pub mod dynamic_updates;
pub mod services;
#[cfg(test)]
pub mod test_utils;

use crate::config::Settings;
use crate::controller::MusicPlayerServer;

// TODO: at some point, we should probably add a panic handler to the daemon to ensure graceful shutdown.

/// Run the daemon
///
/// also initializes the logger, database, and other necessary components.
///
/// # Arguments
///
/// * `settings` - The settings to use.
/// * `db_dir` - The directory where the database is stored.
///              If the directory does not exist, it will be created.
/// * `log_file_path` - The path to the file where logs will be written.
///
/// # Errors
///
/// If the daemon cannot be started, an error is returned.
///
/// # Panics
///
/// Panics if the peer address of the underlying TCP transport cannot be determined.
pub async fn start_daemon(
    settings: Settings,
    db_dir: std::path::PathBuf,
    log_file_path: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    // Throw the given settings into an Arc so we can share settings across threads.
    let settings = Arc::new(settings);

    // check if a server is already running
    if is_server_running(settings.daemon.rpc_port) {
        anyhow::bail!(
            "A server is already running on port {}",
            settings.daemon.rpc_port
        );
    }

    // Initialize the logger, database, and tracing.
    init_logger(settings.daemon.log_level, log_file_path);
    set_database_path(db_dir)?;
    let db = Arc::new(init_database().await?);
    tracing::subscriber::set_global_default(init_tracing())?;

    // Start the music library watcher.
    #[cfg(feature = "dynamic_updates")]
    let guard = dynamic_updates::init_music_library_watcher(
        db.clone(),
        &settings.daemon.library_paths,
        settings.daemon.artist_separator.clone(),
        settings.daemon.genre_separator.clone(),
    )?;

    // Start the audio kernel.
    let audio_kernel = AudioKernelSender::start();
    let server = MusicPlayerServer::new(db.clone(), settings.clone(), audio_kernel.clone());

    // Start the RPC server.
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), settings.daemon.rpc_port);

    let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
    info!("Listening on {}", listener.local_addr());
    listener.config_mut().max_frame_length(usize::MAX);
    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(BaseChannel::with_defaults)
        // Limit channels to 10 per IP.
        .max_channels_per_key(10, |t| t.transport().peer_addr().unwrap().ip())
        // Set up the server's handling of incoming connections.
        // serve is generated by the service attribute.
        // It takes as input any type implementing the generated MusicPlayer trait.
        .map(|channel| channel.execute(server.clone().serve()).for_each(spawn))
        // Max 10 channels.
        // this means that we will only process 10 requests at a time
        // NOTE: if we have issues with concurrency (e.g. deadlocks or data-races),
        //       and have too much of a skill issue to fix it, we can set this number to 1.
        .buffer_unordered(10)
        .for_each(|()| async {})
        .await;

    #[cfg(feature = "dynamic_updates")]
    guard.stop();

    Ok(())
}

/// Initialize a test client, sends and receives messages over a channel / pipe.
/// This is useful for testing the server without needing to start it.
#[must_use]
pub fn init_test_client_server(
    db: Arc<Surreal<Db>>,
    settings: Arc<Settings>,
    audio_kernel: Arc<AudioKernelSender>,
) -> MusicPlayerClient {
    let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

    let server = MusicPlayerServer::new(db, settings, audio_kernel);
    tokio::spawn(
        tarpc::server::BaseChannel::with_defaults(server_transport)
            .execute(server.serve())
            // Handle all requests concurrently.
            .for_each(|response| async move {
                tokio::spawn(response);
            }),
    );

    // MusicPlayerClient is generated by the #[tarpc::service] attribute. It has a constructor `new`
    // that takes a config and any Transport as input.
    MusicPlayerClient::new(tarpc::client::Config::default(), client_transport).spawn()
}

#[cfg(test)]
mod test_client_tests {
    //! Tests for:
    //! - the `init_test_client_server` function
    //! - daemon endpoints that aren't covered in other tests

    use super::*;
    use anyhow::Result;
    use mecomp_core::state::library::LibraryFull;
    use mecomp_storage::{
        db::schemas::{collection::Collection, playlist::Playlist, song::SongChangeSet},
        test_utils::{create_song_with_overrides, init_test_database, SongCase},
    };

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    async fn db() -> Arc<Surreal<Db>> {
        let db = Arc::new(init_test_database().await.unwrap());

        // create a test song, add it to a playlist and collection

        let song_case = SongCase::new(0, vec![0], vec![0], 0, 0);

        // Call the create_song function
        let song = create_song_with_overrides(
            &db,
            song_case,
            SongChangeSet {
                // need to specify overrides so that items are created in the db
                artist: Some(one_or_many::OneOrMany::One("Artist 0".into())),
                album_artist: Some(one_or_many::OneOrMany::One("Artist 0".into())),
                album: Some("Album 0".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // create a playlist with the song
        let playlist = Playlist {
            id: Playlist::generate_id(),
            name: "Playlist 0".into(),
            runtime: song.runtime,
            song_count: 1,
        };

        let result = Playlist::create(&db, playlist).await.unwrap().unwrap();

        Playlist::add_songs(&db, result.id, vec![song.id.clone()])
            .await
            .unwrap();

        // create a collection with the song
        let collection = Collection {
            id: Collection::generate_id(),
            name: "Collection 0".into(),
            runtime: song.runtime,
            song_count: 1,
        };

        let result = Collection::create(&db, collection).await.unwrap().unwrap();

        Collection::add_songs(&db, result.id, vec![song.id])
            .await
            .unwrap();

        return db;
    }

    #[fixture]
    async fn client(#[future] db: Arc<Surreal<Db>>) -> MusicPlayerClient {
        let settings = Arc::new(Settings::default());
        let audio_kernel = AudioKernelSender::start();

        init_test_client_server(db.await, settings, audio_kernel)
    }

    #[tokio::test]
    async fn test_init_test_client_server() {
        let db = Arc::new(init_test_database().await.unwrap());
        let settings = Arc::new(Settings::default());
        let audio_kernel = AudioKernelSender::start();

        let client = init_test_client_server(db, settings, audio_kernel);

        let ctx = tarpc::context::current();
        let response = client.ping(ctx).await.unwrap();

        assert_eq!(response, "pong");

        // ensure that the client is shutdown properly
        drop(client);
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_artist(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_song_get_artist(ctx, library_full.songs.first().unwrap().id.clone().into())
            .await?;

        assert_eq!(response, library_full.artists.into_vec().into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_album(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_song_get_album(ctx, library_full.songs.first().unwrap().id.clone().into())
            .await?
            .unwrap();

        assert_eq!(response, library_full.albums.first().unwrap().clone());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_playlists(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_song_get_playlists(ctx, library_full.songs.first().unwrap().id.clone().into())
            .await?;

        assert_eq!(response, library_full.playlists.into_vec().into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_album_get_artist(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_album_get_artist(ctx, library_full.albums.first().unwrap().id.clone().into())
            .await?;

        assert_eq!(response, library_full.artists.into_vec().into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_album_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_album_get_songs(ctx, library_full.albums.first().unwrap().id.clone().into())
            .await?
            .unwrap();

        assert_eq!(response, library_full.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_artist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_artist_get_songs(ctx, library_full.artists.first().unwrap().id.clone().into())
            .await?
            .unwrap();

        assert_eq!(response, library_full.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_artist_get_albums(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .library_artist_get_albums(ctx, library_full.artists.first().unwrap().id.clone().into())
            .await?
            .unwrap();

        assert_eq!(response, library_full.albums);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playback_volume_toggle_mute(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        client.playback_volume_toggle_mute(ctx).await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_queue_add_list(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let ctx = tarpc::context::current();
        let response = client
            .queue_add_list(
                ctx,
                vec![library_full.songs.first().unwrap().id.clone().into()],
            )
            .await?;

        assert_eq!(response, Ok(()));

        Ok(())
    }

    #[rstest]
    #[case::get(String::from("Playlist 0"))]
    #[case::create(String::from("Playlist 1"))]
    #[tokio::test]
    async fn test_playlist_get_or_create(
        #[future] client: MusicPlayerClient,
        #[case] name: String,
    ) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        // get or create the playlist
        let playlist_id = client
            .playlist_get_or_create(ctx, name.clone())
            .await?
            .unwrap();

        // now get that playlist
        let ctx = tarpc::context::current();
        let playlist = client.playlist_get(ctx, playlist_id).await?.unwrap();

        assert_eq!(playlist.name, name.into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_clone(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        // clone the only playlist in the db
        let ctx = tarpc::context::current();
        let playlist_id = client
            .playlist_clone(
                ctx,
                library_full.playlists.first().unwrap().id.clone().into(),
            )
            .await?
            .unwrap();

        // now get that playlist
        let ctx = tarpc::context::current();
        let playlist = client.playlist_get(ctx, playlist_id).await?.unwrap();

        assert_eq!(playlist.name, "Playlist 0 (copy)".into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        // clone the only playlist in the db
        let response = client
            .playlist_get_songs(
                ctx,
                library_full.playlists.first().unwrap().id.clone().into(),
            )
            .await?
            .unwrap();

        assert_eq!(response, library_full.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_collection_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        // clone the only playlist in the db
        let response = client
            .collection_get_songs(
                ctx,
                library_full.collections.first().unwrap().id.clone().into(),
            )
            .await?
            .unwrap();

        assert_eq!(response, library_full.songs);

        Ok(())
    }
}
