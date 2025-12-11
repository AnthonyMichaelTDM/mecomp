#![deny(clippy::missing_inline_in_public_items)]

//----------------------------------------------------------------------------------------- std lib
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
//--------------------------------------------------------------------------------- other libraries
use log::{error, info};
use persistence::QueueState;
use surrealdb::{Surreal, engine::local::Db};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tracing::Instrument;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    audio::{AudioKernelSender, commands::AudioCommand},
    config::Settings,
    logger::{init_logger, init_tracing},
    udp::{Message, Sender, StateChange},
};
use mecomp_prost::{MusicPlayerClient, TraceInterceptor, server::MusicPlayerServer};
use mecomp_storage::db::{init_database, set_database_path};

pub mod controller;
#[cfg(feature = "dynamic_updates")]
pub mod dynamic_updates;
pub mod persistence;
pub mod services;
pub mod termination;
#[cfg(test)]
pub use mecomp_core::test_utils;

use crate::{controller::MusicPlayer, termination::InterruptReceiver};

/// The maximum number of concurrent requests.
pub const MAX_CONCURRENT_REQUESTS: usize = 4;

/// Event Publisher guard
///
/// This is a newtype for the event publisher that ensures it is stopped when the guard is dropped.
struct EventPublisher {
    dispatcher: Arc<Sender<Message>>,
    event_tx: std::sync::mpsc::Sender<StateChange>,
    handle: tokio::task::JoinHandle<()>,
}

impl EventPublisher {
    /// Start the event publisher
    pub async fn new() -> Self {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let event_publisher = Arc::new(Sender::new().await.unwrap());
        let event_publisher_clone = event_publisher.clone();

        let handle = tokio::task::spawn_blocking(move || {
            while let Ok(event) = event_rx.recv() {
                // re-enter the async context to send the event over UDP
                Handle::current().block_on(async {
                    if let Err(e) = event_publisher_clone
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
/// * `state_file_path` - The path to the file where the queue state restored from / saved to.
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
    db_dir: PathBuf,
    log_file_path: Option<PathBuf>,
    state_file_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    // Throw the given settings into an Arc so we can share settings across threads.
    let settings = Arc::new(settings);

    // Initialize the logger, database, and tracing.
    init_logger(settings.daemon.log_level, log_file_path);
    set_database_path(db_dir)?;
    tracing::subscriber::set_global_default(init_tracing())?;
    log::debug!("initialized logging");

    // bind to `localhost:{rpc_port}`, we do this as soon as possible to minimize perceived startup delay
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), settings.daemon.rpc_port);
    let listener = TcpListener::bind(server_addr).await?;
    info!(
        "Listening on {}, preparing to handle requests",
        listener.local_addr()?
    );
    let incoming = TcpListenerStream::new(listener);

    // start initializing the database asynchronously
    let db_task = tokio::task::spawn(init_database());

    // initialize the termination handler
    let (terminator, interrupt_rx) = termination::create_termination();
    log::debug!("initialized terminator");

    // initialize the event publisher
    let event_publisher_guard = EventPublisher::new().await;
    log::debug!("initialized event publisher");

    // Start the audio kernel.
    let audio_kernel = AudioKernelSender::start(event_publisher_guard.event_tx.clone());
    log::debug!("initialized audio kernel");

    // optionally restore the queue state
    if let Some(state_path) = &state_file_path {
        info!("Restoring queue state from {}", state_path.display());
        match QueueState::load_from_file(state_path) {
            Ok(state) => state.restore_to(&audio_kernel),
            Err(e) => error!("Failed to restore queue state: {e}"),
        }
    }

    // join the db initialization task
    let db = Arc::new(db_task.await??);
    log::debug!("initialized database");

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

    // Initialize the server state.
    let state = MusicPlayer::new(
        db.clone(),
        settings.clone(),
        audio_kernel.clone(),
        event_publisher_guard.dispatcher.clone(),
        terminator.clone(),
        interrupt_rx.resubscribe(),
    );

    // Start the daemon server.
    if let Err(e) = run_daemon(incoming, state, interrupt_rx.resubscribe()).await {
        error!("Error running daemon: {e}");
    }

    #[cfg(feature = "dynamic_updates")]
    guard.stop();

    // send a shutdown event to all clients (ignore errors)
    let _ = event_publisher_guard
        .dispatcher
        .send(Message::Event(mecomp_core::udp::Event::DaemonShutdown))
        .await;

    if let Some(state_path) = &state_file_path {
        info!("Persisting queue state to {}", state_path.display());
        let _ = QueueState::retrieve(&audio_kernel)
            .await
            .and_then(|state| state.save_to_file(state_path))
            .inspect_err(|e| error!("Failed to persist queue state: {e}"));
    }

    log::info!("Cleanup complete");

    Ok(())
}

/// Run the daemon with the given settings, database, and state file path.
/// Does not handle setup or teardown, just runs the server.
async fn run_daemon(
    incoming: TcpListenerStream,
    state: MusicPlayer,
    mut interrupt_rx: InterruptReceiver,
) -> anyhow::Result<()> {
    // Start the RPC server listening to the given stream of `incoming` data
    let svc = MusicPlayerServer::new(state);

    let shutdown_future = async move {
        // Wait for the server to be stopped.
        // This will be triggered by the signal handler.
        match interrupt_rx.wait().await {
            Ok(termination::Interrupted::UserInt) => info!("Stopping server per user request"),
            Ok(termination::Interrupted::OsSigInt) => {
                info!("Stopping server because of an os sig int");
            }
            Ok(termination::Interrupted::OsSigTerm) => {
                info!("Stopping server because of an os sig term");
            }
            Ok(termination::Interrupted::OsSigQuit) => {
                info!("Stopping server because of an os sig quit");
            }
            Err(e) => error!("Stopping server because of an unexpected error: {e}"),
        }
    };

    info!("Daemon is ready to handle requests");

    Server::builder()
        .trace_fn(|r| tracing::trace_span!("grpc", "request" = %r.uri()))
        .add_service(svc)
        .serve_with_incoming_shutdown(incoming, shutdown_future)
        .await?;

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
    // initialize the event publisher
    let event_publisher = Arc::new(Sender::new().await?);
    // initialize the termination handler
    let (terminator, mut interrupt_rx) = termination::create_termination();

    // Build the service implementation
    let server = MusicPlayer::new(
        db,
        settings.clone(),
        audio_kernel.clone(),
        event_publisher.clone(),
        terminator,
        interrupt_rx.resubscribe(),
    );

    // Bind an ephemeral local port for the in-process server
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let incoming = TcpListenerStream::new(listener);

    // Create the gRPC service
    let svc = MusicPlayerServer::new(server);

    // Spawn the server with shutdown triggered by interrupt receiver
    tokio::spawn(async move {
        let shutdown_future = async move {
            let _ = interrupt_rx.wait().await;
            info!("Stopping test server...");
            audio_kernel.send(AudioCommand::Exit);
            let _ = event_publisher
                .send(Message::Event(mecomp_core::udp::Event::DaemonShutdown))
                .await;
            info!("Test server stopped");
        };

        if let Err(e) = Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(incoming, shutdown_future)
            .await
        {
            error!("Error running test server: {e}");
        }
    });

    // Connect a client to the local server
    let endpoint = format!("http://{local_addr}");
    let endpoint = tonic::transport::Channel::from_shared(endpoint)?.connect_lazy();

    let client =
        mecomp_prost::client::MusicPlayerClient::with_interceptor(endpoint, TraceInterceptor {});
    Ok(client)
}

#[cfg(test)]
mod test_client_tests {
    //! Tests for:
    //! - the `init_test_client_server` function
    //! - daemon endpoints that aren't covered in other tests

    use std::io::{Read, Write};

    use super::*;
    use anyhow::Result;
    use mecomp_core::errors::{BackupError, SerializableLibraryError};
    use mecomp_prost::{
        DynamicPlaylist, DynamicPlaylistChangeSet, DynamicPlaylistCreateRequest,
        DynamicPlaylistUpdateRequest, LibraryFull, Path, PlaylistExportRequest,
        PlaylistImportRequest, PlaylistName, PlaylistRenameRequest, RecordIdList,
    };
    use mecomp_storage::{
        db::schemas::{
            collection::Collection,
            dynamic::query::{Compile, Query},
            playlist::Playlist,
            song::SongChangeSet,
        },
        test_utils::{SongCase, create_song_with_overrides, init_test_database},
    };

    use pretty_assertions::{assert_eq, assert_str_eq};
    use rstest::{fixture, rstest};
    use tonic::Code;

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
                artist: Some("Artist 0".to_string().into()),
                album_artist: Some("Artist 0".to_string().into()),
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

        let mut client = init_test_client_server(db, settings, audio_kernel)
            .await
            .unwrap();

        let response = client.ping(()).await.unwrap().into_inner().message;

        assert_eq!(response, "pong");

        // ensure that the client is shutdown properly
        drop(client);
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_artist(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_brief = client.library_brief(()).await?.into_inner();

        let response = client
            .library_song_get_artists(library_brief.songs.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .artists;

        assert_eq!(response, library_brief.artists);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_album(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_brief = client.library_brief(()).await?.into_inner();

        let response = client
            .library_song_get_album(library_brief.songs.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .album
            .unwrap();

        assert_eq!(response, library_brief.albums.first().unwrap().clone());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_song_get_playlists(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_full: LibraryFull = client.library_full(()).await?.into_inner();

        let response = client
            .library_song_get_playlists(library_full.songs.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .playlists;

        assert_eq!(response, library_full.playlists);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_album_get_artist(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library = client.library_brief(()).await?.into_inner();

        let response = client
            .library_album_get_artists(library.albums.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .artists;

        assert_eq!(response, library.artists);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_album_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_brief = client.library_brief(()).await?.into_inner();

        let response = client
            .library_album_get_songs(library_brief.albums.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .songs;

        assert_eq!(response, library_brief.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_artist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library = client.library_brief(()).await?.into_inner();

        let response = client
            .library_artist_get_songs(library.artists.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .songs;

        assert_eq!(response, library.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_library_artist_get_albums(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library = client.library_brief(()).await?.into_inner();

        let response = client
            .library_artist_get_albums(library.artists.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .albums;

        assert_eq!(response, library.albums);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playback_toggle_mute(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        client.playback_toggle_mute(()).await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playback_stop(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        client.playback_stop(()).await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_queue_add_list(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_full: LibraryFull = client.library_full(()).await?.into_inner();

        let response = client
            .queue_add_list(RecordIdList::new(vec![
                library_full.songs.first().unwrap().id.clone().into(),
            ]))
            .await;

        assert!(response.is_ok());

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
        let mut client = client.await;

        // get or create the playlist
        let playlist_id = client
            .playlist_get_or_create(PlaylistName::new(name.clone()))
            .await?
            .into_inner();

        // now get that playlist
        let playlist = client
            .library_playlist_get(playlist_id.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(playlist.name, name);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_clone(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_full: LibraryFull = client.library_full(()).await?.into_inner();

        // clone the only playlist in the db
        let playlist_id = client
            .playlist_clone(library_full.playlists.first().unwrap().id.ulid())
            .await?
            .into_inner();

        // now get that playlist
        let playlist = client
            .library_playlist_get(playlist_id.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(playlist.name, "Playlist 0 (copy)");

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library = client.library_brief(()).await?.into_inner();

        // clone the only playlist in the db
        let response = client
            .library_playlist_get_songs(library.playlists.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .songs;

        assert_eq!(response, library.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_rename(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library_full: LibraryFull = client.library_full(()).await?.into_inner();

        let target = library_full.playlists.first().unwrap();

        let response = client
            .playlist_rename(PlaylistRenameRequest::new(target.id.id.clone(), "New Name"))
            .await
            .unwrap()
            .into_inner();

        let expected = mecomp_prost::Playlist {
            name: "New Name".into(),
            ..target.clone()
        };

        assert_eq!(response, expected.clone().into());

        let response = client
            .library_playlist_get(target.id.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(response, expected);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_collection_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let library = client.library_brief(()).await?.into_inner();

        // clone the only playlist in the db
        let response = client
            .library_collection_get_songs(library.collections.first().unwrap().id.ulid())
            .await?
            .into_inner()
            .songs;

        assert_eq!(response, library.songs);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_create(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let response = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                query,
            ))
            .await;

        assert!(response.is_ok());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_list(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                query,
            ))
            .await?
            .into_inner();

        let response = client
            .library_dynamic_playlists(())
            .await?
            .into_inner()
            .playlists;

        assert_eq!(response.len(), 1);
        assert_eq!(response.first().unwrap().id, dynamic_playlist_id);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_update(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                &query,
            ))
            .await?
            .into_inner();

        let response = client
            .dynamic_playlist_update(DynamicPlaylistUpdateRequest::new(
                dynamic_playlist_id.id.clone(),
                DynamicPlaylistChangeSet::new().name("Dynamic Playlist 1"),
            ))
            .await?
            .into_inner();

        let expected = DynamicPlaylist {
            id: dynamic_playlist_id.clone().into(),
            name: "Dynamic Playlist 1".into(),
            query: query.clone().compile_for_storage(),
        };

        assert_eq!(response, expected.clone());

        let response = client
            .library_dynamic_playlist_get(dynamic_playlist_id.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(response, expected);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_remove(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                query,
            ))
            .await?
            .into_inner();

        let response = client
            .dynamic_playlist_remove(dynamic_playlist_id.ulid())
            .await;

        assert!(response.is_ok());

        let response = client
            .library_dynamic_playlists(())
            .await?
            .into_inner()
            .playlists;

        assert_eq!(response.len(), 0);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_get(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                &query,
            ))
            .await?
            .into_inner();

        let response = client
            .library_dynamic_playlist_get(dynamic_playlist_id.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(response.name, "Dynamic Playlist 0");
        assert_eq!(response.query, query.compile_for_storage());

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_get_songs(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let dynamic_playlist_id = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                query,
            ))
            .await?
            .into_inner();

        let response = client
            .library_dynamic_playlist_get_songs(dynamic_playlist_id.ulid())
            .await?
            .into_inner()
            .songs;

        assert_eq!(response.len(), 1);

        Ok(())
    }

    // Dynamic Playlist Import Tests
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "dynamic playlist name,query")?;
        writeln!(file, "Dynamic Playlist 0,artist CONTAINS \"Artist 0\"")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile_path))
            .await?
            .into_inner()
            .playlists;

        let expected = DynamicPlaylist {
            id: response[0].id.clone(),
            name: "Dynamic Playlist 0".into(),
            query: query.compile_for_storage(),
        };

        assert_eq!(response, vec![expected]);

        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_nonexistent(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let tmpfile_path = "/this/path/does/not/exist.csv";

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile_path))
            .await;
        assert!(response.is_err(), "response: {response:?}");
        assert_eq!(
            response.unwrap_err().message(),
            format!("Backup Error: The file \"{tmpfile_path}\" does not exist")
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_wrong_extension(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.txt")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_err(), "response: {response:?}");
        assert_str_eq!(
            response.unwrap_err().message(),
            format!(
                "Backup Error: The file \"{}\" has the wrong extension, expected: csv",
                tmpfile.path().display()
            )
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_is_directory(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::tempdir()?;

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert_eq!(response.code(), Code::InvalidArgument);
        assert_str_eq!(
            response.message(),
            format!(
                "Backup Error: {} is a directory, not a file",
                tmpfile.path().display()
            )
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_invalid_format(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "artist,album,album_artist,title")?;

        let tmpfile_path = tmpfile.path().to_path_buf();

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile_path))
            .await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert_eq!(response.code(), Code::InvalidArgument);
        assert_str_eq!(
            response.message(),
            "Backup Error: No valid playlists were found in the csv file."
        );
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_import_file_invalid_query(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        // write a csv file to the temp file
        let mut file = tmpfile.reopen()?;
        writeln!(file, "dynamic playlist name,query")?;
        writeln!(file, "Dynamic Playlist 0,artist CONTAINS \"Artist 0\"")?;
        writeln!(file, "Dynamic Playlist 1,artist CONTAINS \"")?;

        let response = client
            .dynamic_playlist_import(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        let expected = SerializableLibraryError::BackupError(
            BackupError::InvalidDynamicPlaylistQuery(
                String::from(
                    "failed to parse field at 16, (inner: Mismatch at 16: seq [114, 101, 108, 101, 97, 115, 101, 95, 121, 101, 97, 114] expect: 114, found: 34)",
                ),
                2,
            ),
        );
        assert_eq!(response.code(), Code::Internal);
        assert_str_eq!(
            response.message(),
            expected.to_string(),
            "response: {response:?}"
        );
        Ok(())
    }

    // Dynamic Playlist Export Tests
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("test.csv");

        let query: Query = "artist CONTAINS \"Artist 0\"".parse()?;
        let _ = client
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(
                "Dynamic Playlist 0",
                query.clone(),
            ))
            .await?;

        let expected = r#"dynamic playlist name,query
Dynamic Playlist 0,"artist CONTAINS ""Artist 0"""
"#;

        let response = client
            .dynamic_playlist_export(Path::new(path.clone()))
            .await;
        assert!(response.is_ok(), "response: {response:?}");

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
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.csv")?;

        let response = client
            .dynamic_playlist_export(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_ok(), "response: {response:?}");
        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export_file_is_directory(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::tempdir()?;

        let response = client
            .dynamic_playlist_export(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert_eq!(response.code(), Code::InvalidArgument);
        let expected = SerializableLibraryError::BackupError(BackupError::PathIsDirectory(
            tmpfile.path().to_path_buf(),
        ));
        assert_str_eq!(
            response.message(),
            expected.to_string(),
            "response: {response:?}"
        );

        Ok(())
    }
    #[rstest]
    #[tokio::test]
    async fn test_dynamic_playlist_export_file_invalid_extension(
        #[future] client: MusicPlayerClient,
    ) -> Result<()> {
        let mut client = client.await;

        let tmpfile = tempfile::NamedTempFile::with_suffix("dps.txt")?;

        let response = client
            .dynamic_playlist_export(Path::new(tmpfile.path()))
            .await;
        assert!(response.is_err(), "response: {response:?}");
        let err = response.unwrap_err();
        let expected = SerializableLibraryError::BackupError(BackupError::WrongExtension(
            tmpfile.path().to_path_buf(),
            String::from("csv"),
        ))
        .to_string();
        assert_str_eq!(&err.message(), &expected,);

        Ok(())
    }

    // Playlist import test
    #[rstest]
    #[tokio::test]
    async fn test_playlist_import(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

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

        let response = client
            .playlist_import(PlaylistImportRequest::new(tmpfile_path))
            .await;
        assert!(response.is_ok());
        let response = response.unwrap().into_inner();

        let playlist = client
            .library_playlist_get(response.ulid())
            .await?
            .into_inner()
            .playlist
            .unwrap();

        assert_eq!(playlist.name, "Imported Playlist");
        assert_eq!(playlist.song_count, 1);

        let songs = client
            .library_playlist_get_songs(response.ulid())
            .await?
            .into_inner()
            .songs;
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].path, "/path/to/song.mp3");

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_playlist_export(#[future] client: MusicPlayerClient) -> Result<()> {
        let mut client = client.await;

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("test.m3u");

        let library_full: LibraryFull = client.library_full(()).await?.into_inner();

        let playlist = library_full.playlists[0].clone();

        let response = client
            .playlist_export(PlaylistExportRequest::new(
                playlist.id.clone(),
                path.clone(),
            ))
            .await;
        assert!(response.is_ok(), "response: {response:?}");

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
