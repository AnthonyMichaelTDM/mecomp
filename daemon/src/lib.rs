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
        .map(|channel| {
            let server = MusicPlayerServer::new(db.clone(), settings.clone(), audio_kernel.clone());
            channel.execute(server.serve()).for_each(spawn)
        })
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
    use super::*;
    use mecomp_storage::test_utils::init_test_database;

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
}
