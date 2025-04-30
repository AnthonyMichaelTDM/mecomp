#![deny(clippy::missing_inline_in_public_items)]

//----------------------------------------------------------------------------------------- std lib
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
//--------------------------------------------------------------------------------- other libraries
use futures::{future, prelude::*};
use log::{error, info};
use surrealdb::{Surreal, engine::local::Db};
use tarpc::{
    self,
    serde_transport::tcp,
    server::{BaseChannel, Channel as _, incoming::Incoming as _},
    tokio_serde::formats::Json,
};
use tokio::{runtime::Handle, sync::RwLock};
use tracing::Instrument;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{AudioKernelSender, commands::AudioCommand},
    config::Settings,
    is_server_running,
    logger::{init_logger, init_tracing},
    rpc::{MusicPlayer as _, MusicPlayerClient},
    udp::{Message, Sender, StateChange},
};
use mecomp_storage::db::{init_database, set_database_path};

async fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if size_of::<F>() > 1024 {
        // if the future is too big, box it before spawning
        let fut = Box::pin(fut);
        tokio::spawn(fut);
    } else {
        // if the future is small enough, spawn it directly
        tokio::spawn(fut);
    }
}

pub mod controller;
#[cfg(feature = "dynamic_updates")]
pub mod dynamic_updates;
pub mod services;
pub mod termination;
#[cfg(test)]
pub use mecomp_core::test_utils;

use crate::controller::MusicPlayerServer;

/// The number of connections per IP address.
const CHANNELS_PER_IP: u32 = 10;
/// The maximum number of concurrent requests.
pub const MAX_CONCURRENT_REQUESTS: usize = 4;

/// Event Publisher guard
///
/// This is a newtype for the event publisher that ensures it is stopped when the guard is dropped.
struct EventPublisher {
    dispatcher: Arc<RwLock<Sender<Message>>>,
    event_tx: std::sync::mpsc::Sender<StateChange>,
    handle: tokio::task::JoinHandle<()>,
}

impl EventPublisher {
    /// Start the event publisher
    pub async fn new() -> Self {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let event_publisher = Arc::new(RwLock::const_new(Sender::new().await.unwrap()));
        let event_publisher_clone = event_publisher.clone();

        let handle = tokio::task::spawn_blocking(move || {
            while let Ok(event) = event_rx.recv() {
                // re-enter the async context to send the event over UDP
                Handle::current().block_on(async {
                    if let Err(e) = event_publisher_clone
                        .read()
                        .await
                        .send(Message::StateChange(event))
                        .await
                    {
                        error!("Failed to send event over UDP: {e}");
                    }
                });
            }
        })
        .instrument(tracing::info_span!("event_publisher"));

        Self {
            dispatcher: event_publisher,
            event_tx,
            handle: handle.into_inner(),
        }
    }
}

impl Drop for EventPublisher {
    fn drop(&mut self) {
        // Stop the event publisher thread
        self.handle.abort();
    }
}

// TODO: at some point, we should probably add a panic handler to the daemon to ensure graceful shutdown.

/// Run the daemon
///
/// also initializes the logger, database, and other necessary components.
///
/// # Arguments
///
/// * `settings` - The settings to use.
/// * `db_dir` - The directory where the database is stored.
///   If the directory does not exist, it will be created.
/// * `log_file_path` - The path to the file where logs will be written.
///
/// # Errors
///
/// If the daemon cannot be started, an error is returned.
///
/// # Panics
///
/// Panics if the peer address of the underlying TCP transport cannot be determined.
#[inline]
#[allow(clippy::redundant_pub_crate)]
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

    // initialize the termination handler
    let (terminator, mut interrupt_rx) = termination::create_termination();

    // Start the music library watcher.
    #[cfg(feature = "dynamic_updates")]
    let guard = dynamic_updates::init_music_library_watcher(
        db.clone(),
        &settings.daemon.library_paths,
        settings.daemon.artist_separator.clone(),
        settings.daemon.protected_artist_names.clone(),
        settings.daemon.genre_separator.clone(),
        interrupt_rx.resubscribe(),
    )?;

    // initialize the event publisher
    let event_publisher_guard = EventPublisher::new().await;

    // Start the audio kernel.
    let audio_kernel = AudioKernelSender::start(event_publisher_guard.event_tx.clone());

    // Initialize the server.
    let server = MusicPlayerServer::new(
        db.clone(),
        settings.clone(),
        audio_kernel.clone(),
        event_publisher_guard.dispatcher.clone(),
        terminator.clone(),
        interrupt_rx.resubscribe(),
    );

    // Start the RPC server.
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), settings.daemon.rpc_port);

    let mut listener = match tcp::listen(&server_addr, Json::default).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to start server: {e}");

            #[cfg(feature = "dynamic_updates")]
            guard.stop();

            return Err(anyhow::anyhow!("Failed to start server: {e}"));
        }
    };
    info!("Listening on {}", listener.local_addr());
    listener.config_mut().max_frame_length(usize::MAX);
    let server_handle = listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(BaseChannel::with_defaults)
        // Limit channels to 10 per IP.
        .max_channels_per_key(CHANNELS_PER_IP, |t| t.transport().peer_addr().unwrap().ip())
        // Set up the server's handling of incoming connections.
        // serve is generated by the service attribute.
        // It takes as input any type implementing the generated MusicPlayer trait.
        .map(|channel| channel.execute(server.clone().serve()).for_each(spawn))
        // Max 4 channels.
        // this means that we will only process 4 requests at a time
        // NOTE: if we have issues with concurrency (e.g. deadlocks or data-races),
        //       and have too much of a skill issue to fix it, we can set this number to 1.
        .buffer_unordered(MAX_CONCURRENT_REQUESTS)
        .for_each(async |()| {})
        // make it fused so we can stop it later
        .fuse();

    // run the server until it is terminated
    tokio::select! {
        () = server_handle => {
            error!("Server stopped unexpectedly");
        },
        // Wait for the server to be stopped.
        // This will be triggered by the signal handler.
        reason = interrupt_rx.wait() => {
            match reason {
                Ok(termination::Interrupted::UserInt) => info!("Stopping server per user request"),
                Ok(termination::Interrupted::OsSigInt) => info!("Stopping server because of an os sig int"),
                Ok(termination::Interrupted::OsSigTerm) => info!("Stopping server because of an os sig term"),
                Ok(termination::Interrupted::OsSigQuit) => info!("Stopping server because of an os sig quit"),
                Err(e) => error!("Stopping server because of an unexpected error: {e}"),
            }
        }
    }

    #[cfg(feature = "dynamic_updates")]
    guard.stop();

    // send a shutdown event to all clients (ignore errors)
    let _ = event_publisher_guard
        .dispatcher
        .read()
        .await
        .send(Message::Event(mecomp_core::udp::Event::DaemonShutdown))
        .await;

    log::info!("Cleanup complete, exiting...");

    Ok(())
}

/// Initialize a test client, sends and receives messages over a channel / pipe.
/// This is useful for testing the server without needing to start it.
///
/// # Errors
///
/// Errors if the event publisher cannot be created.
#[inline]
pub async fn init_test_client_server(
    db: Arc<Surreal<Db>>,
    settings: Arc<Settings>,
    audio_kernel: Arc<AudioKernelSender>,
) -> anyhow::Result<MusicPlayerClient> {
    let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

    let event_publisher = Arc::new(RwLock::new(Sender::new().await?));
    // initialize the termination handler
    let (terminator, mut interrupt_rx) = termination::create_termination();
    #[allow(clippy::redundant_pub_crate)]
    tokio::spawn(async move {
        let server = MusicPlayerServer::new(
            db,
            settings,
            audio_kernel.clone(),
            event_publisher.clone(),
            terminator,
            interrupt_rx.resubscribe(),
        );
        tokio::select! {
            () = tarpc::server::BaseChannel::with_defaults(server_transport)
                .execute(server.serve())
                // Handle all requests concurrently.
                .for_each(async |response| {
                    tokio::spawn(response);
                }) => {},
            // Wait for the server to be stopped.
            _ = interrupt_rx.wait() => {
                // Stop the server.
                info!("Stopping server...");
                audio_kernel.send(AudioCommand::Exit);
                let _ = event_publisher.read().await.send(Message::Event(mecomp_core::udp::Event::DaemonShutdown)).await;
                info!("Server stopped");
            }
        }
    });

    // MusicPlayerClient is generated by the #[tarpc::service] attribute. It has a constructor `new`
    // that takes a config and any Transport as input.
    Ok(MusicPlayerClient::new(tarpc::client::Config::default(), client_transport).spawn())
}

#[cfg(test)]
mod test_client_tests {
    //! Tests for:
    //! - the `init_test_client_server` function
    //! - daemon endpoints that aren't covered in other tests

    use std::io::{Read, Write};

    use super::*;
    use anyhow::Result;
    use mecomp_core::{
        errors::{BackupError, SerializableLibraryError},
        state::library::LibraryFull,
    };
    use mecomp_storage::{
        db::schemas::{
            collection::Collection,
            dynamic::{DynamicPlaylist, DynamicPlaylistChangeSet, query::Query},
            playlist::Playlist,
            song::SongChangeSet,
        },
        test_utils::{SongCase, create_song_with_overrides, init_test_database},
    };

    use pretty_assertions::{assert_eq, assert_str_eq};
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
                path: Some("/path/to/song.mp3".into()),
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
        let (tx, _) = std::sync::mpsc::channel();
        let audio_kernel = AudioKernelSender::start(tx);

        init_test_client_server(db.await, settings, audio_kernel)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_init_test_client_server() {
        let db = Arc::new(init_test_database().await.unwrap());
        let settings = Arc::new(Settings::default());
        let (tx, _) = std::sync::mpsc::channel();
        let audio_kernel = AudioKernelSender::start(tx);

        let client = init_test_client_server(db, settings, audio_kernel)
            .await
            .unwrap();

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
    async fn test_playback_stop(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        client.playback_stop(ctx).await?;
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

        assert_eq!(playlist.name, name);

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

        assert_eq!(playlist.name, "Playlist 0 (copy)");

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
    async fn test_playlist_rename(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let target = library_full.playlists.first().unwrap();

        let ctx = tarpc::context::current();
        let response = client
            .playlist_rename(ctx, target.id.clone().into(), "New Name".into())
            .await?;

        let expected = Playlist {
            name: "New Name".into(),
            ..target.clone()
        };

        assert_eq!(response, Ok(expected.clone()));

        let ctx = tarpc::context::current();
        let response = client
            .playlist_get(ctx, target.id.clone().into())
            .await?
            .unwrap();

        assert_eq!(response, expected);
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

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_create(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let response = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query)
            .await?;

        assert!(response.is_ok());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_list(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query)
            .await?
            .unwrap();

        let ctx = tarpc::context::current();
        let response = client.dynamic_playlist_list(ctx).await?;

        assert_eq!(response.len(), 1);
        assert_eq!(response.first().unwrap().id, dynamic_playlist_id.into());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_update(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query.clone())
            .await?
            .unwrap();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_update(
                ctx,
                dynamic_playlist_id.clone(),
                DynamicPlaylistChangeSet::new().name("Dynamic Playlist 1"),
            )
            .await?;

        let expected = DynamicPlaylist {
            id: dynamic_playlist_id.clone().into(),
            name: "Dynamic Playlist 1".into(),
            query: query.clone(),
        };

        assert_eq!(response, Ok(expected.clone()));

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_get(ctx, dynamic_playlist_id)
            .await?
            .unwrap();

        assert_eq!(response, expected);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_remove(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query)
            .await?
            .unwrap();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_remove(ctx, dynamic_playlist_id)
            .await?;

        assert_eq!(response, Ok(()));

        let ctx = tarpc::context::current();
        let response = client.dynamic_playlist_list(ctx).await?;

        assert_eq!(response.len(), 0);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_get(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query.clone())
            .await?
            .unwrap();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_get(ctx, dynamic_playlist_id)
            .await?
            .unwrap();

        assert_eq!(response.name, "Dynamic Playlist 0");
        assert_eq!(response.query, query);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let ctx = tarpc::context::current();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query)
            .await?
            .unwrap();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_get_songs(ctx, dynamic_playlist_id)
            .await?
            .unwrap();

        assert_eq!(response.len(), 1);

        Ok(())
    }

    // Dynamic Playlist Import Tests
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "dynamic playlist name,query")?;
        writeln!(file, "Dynamic Playlist 0,artist CONTAINS \"Artist 0\"")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let ctx = tarpc::context::current();
        let response = client.dynamic_playlist_import(ctx, tmpfile_path).await??;

        let expected = DynamicPlaylist {
            id: response[0].id.clone(),
            name: "Dynamic Playlist 0".into(),
            query: query.clone(),
        };

        assert_eq!(response, vec![expected]);

        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_nonexistent(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let tmpfile_path = "/this/path/does/not/exist.csv";

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_import(ctx, tmpfile_path.into())
            .await?;
        assert!(response.is_err(), "response: {response:?}");
        assert_eq!(
            response.unwrap_err().to_string(),
            format!("Backup Error: The file \"{tmpfile_path}\" does not exist")
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_wrong_extension(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.txt")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_import(ctx, tmpfile_path.clone())
            .await?;
        assert!(response.is_err(), "response: {response:?}");
        assert_eq!(
            response.unwrap_err().to_string(),
            format!(
                "Backup Error: The file \"{}\" has the wrong extension, expected: csv",
                tmpfile_path.display()
            )
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_is_directory(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::tempdir()?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_import(ctx, tmpfile_path.clone())
            .await?;
        assert!(response.is_err());
        assert_eq!(
            response.unwrap_err().to_string(),
            format!(
                "Backup Error: {} is a directory, not a file",
                tmpfile_path.display()
            )
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_invalid_format(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let ctx = tarpc::context::current();
        let response = client.dynamic_playlist_import(ctx, tmpfile_path).await?;
        assert!(response.is_err());
        assert_eq!(
            response.unwrap_err().to_string(),
            "Backup Error: No valid playlists were found in the csv file."
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_invalid_query(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "dynamic playlist name,query")?;
        writeln!(file, "Dynamic Playlist 0,artist CONTAINS \"Artist 0\"")?;
        writeln!(file, "Dynamic Playlist 1,artist CONTAINS \"")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let ctx = tarpc::context::current();
        let response = client.dynamic_playlist_import(ctx, tmpfile_path).await?;
        assert!(
            matches!(
                response,
                Err(SerializableLibraryError::BackupError(
                    BackupError::InvalidDynamicPlaylistQuery(_, 2)
                ))
            ),
            "response: {response:?}"
        );
        Ok(())
    }

    // Dynamic Playlist Export Tests
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("test.csv");

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;
        let ctx = tarpc::context::current();
        let _ = client
            .dynamic_playlist_create(ctx, "Dynamic Playlist 0".into(), query.clone())
            .await?
            .unwrap();

        let expected = r#"dynamic playlist name,query
Dynamic Playlist 0,"artist CONTAINS ""Artist 0"""
"#;

        let response = client.dynamic_playlist_export(ctx, path.clone()).await?;
        assert_eq!(response, Ok(()));

        let mut file = std::fs::File::open(path.clone())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        assert_str_eq!(contents, expected);

        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export_file_exists(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_export(ctx, tmpfile.path().to_path_buf())
            .await?;
        assert!(matches!(response, Ok(())), "response: {response:?}");
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export_file_is_directory(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::tempdir()?;

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_export(ctx, tmpfile.path().to_path_buf())
            .await?;
        assert!(
            matches!(
                response,
                Err(SerializableLibraryError::BackupError(
                    BackupError::PathIsDirectory(_)
                ))
            ),
            "response: {response:?}"
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export_file_invalid_extension(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.txt")?;

        let ctx = tarpc::context::current();
        let response = client
            .dynamic_playlist_export(ctx, tmpfile.path().to_path_buf())
            .await?;
        assert!(response.is_err(), "response: {response:?}");
        let err = response.unwrap_err();
        assert!(
            matches!(
                &err,
                SerializableLibraryError::BackupError(
                    BackupError::WrongExtension(_, expected_extension)
                ) if expected_extension == "csv"
            ),
            "response: {err:?}"
        );

        Ok(())
    }

    // Playlist import test
    #[rstest]
    #[tokio::test]
    async fn test_playlist_import(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("pl.m3u")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        write!(
            file,
            r"#EXTM3U
#EXTINF:123,Sample Artist - Sample title
/path/to/song.mp3
"
        )?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let ctx = tarpc::context::current();
        let response = client.playlist_import(ctx, tmpfile_path, None).await?;
        assert!(response.is_ok());
        let response = response.unwrap();

        let ctx = tarpc::context::current();
        let playlist = client.playlist_get(ctx, response.clone()).await?.unwrap();

        assert_eq!(playlist.name, "Imported Playlist");
        assert_eq!(playlist.song_count, 1);

        let ctx = tarpc::context::current();
        let songs = client
            .playlist_get_songs(ctx, response.clone())
            .await?
            .unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].path.to_string_lossy(), "/path/to/song.mp3");

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_export(#[future] client: MusicPlayerClient) -> Result<()> {
        let client = client.await;

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("test.m3u");

        let ctx = tarpc::context::current();
        let library_full: LibraryFull = client.library_full(ctx).await??;

        let playlist = library_full.playlists[0].clone();

        let response = client
            .playlist_export(ctx, playlist.id.clone().into(), path.clone())
            .await?;
        assert_eq!(response, Ok(()));

        let mut file = std::fs::File::open(path.clone())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        assert_str_eq!(
            contents,
            r"#EXTM3U

#PLAYLIST:Playlist 0

#EXTINF:120,Song 0 - Artist 0
#EXTGENRE:Genre 0
#EXTALB:Artist 0
/path/to/song.mp3

"
        );

        Ok(())
    }
}
