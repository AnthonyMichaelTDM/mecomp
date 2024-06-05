//----------------------------------------------------------------------------------------- std lib
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
//--------------------------------------------------------------------------------- other libraries
use futures::{future, prelude::*};
use log::info;
use tarpc::{
    self, client,
    server::{incoming::Incoming as _, BaseChannel, Channel as _},
    tokio_serde::formats::Bincode,
};
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
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
pub mod errors;
pub mod services;

use crate::config::DaemonSettings;
use crate::controller::MusicPlayerServer;

/// Run the daemon
///
/// also initializes the logger, database, and other necessary components.
///
/// # Arguments
///
/// * `log_level` - The log level to use.
/// * `settings` - The settings to use.
///
/// # Errors
///
/// If the daemon cannot be started, an error is returned.
///
/// # Panics
///
/// Panics if the peer address of the underlying TCP transport cannot be determined.
pub async fn start_daemon(
    log_level: log::LevelFilter,
    settings: DaemonSettings,
) -> anyhow::Result<()> {
    // Throw the given settings into an Arc so we can share settings across threads.
    let settings = Arc::new(settings);

    // Initialize the logger, database, and tracing.
    init_logger(log_level);
    set_database_path(settings.db_path.clone())?;
    let db = Arc::new(init_database().await?);
    tracing::subscriber::set_global_default(init_tracing())?;

    // Start the music library watcher.
    #[cfg(feature = "dynamic_updates")]
    let _watcher = dynamic_updates::init_music_library_watcher(
        db.clone(),
        &settings.library_paths,
        settings.artist_separator.clone(),
        settings.genre_separator.clone(),
    )?;

    // Start the RPC server.
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), settings.rpc_port);

    let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Bincode::default).await?;
    info!("Listening on {}", listener.local_addr());
    listener.config_mut().max_frame_length(usize::MAX);
    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(BaseChannel::with_defaults)
        // Limit channels to 1 per IP.
        .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
        // Set up the server's handling of incoming connections.
        // serve is generated by the service attribute.
        // It takes as input any type implementing the generated MusicPlayer trait.
        .map(|channel| {
            let server = MusicPlayerServer::new(
                channel.transport().peer_addr().unwrap(),
                db.clone(),
                settings.clone(),
            );
            channel.execute(server.serve()).for_each(spawn)
        })
        // Max 10 channels.
        // this means that we will only process 10 requests at a time
        // NOTE: if we have issues with concurrency (e.g. deadlocks or data-races),
        //       and have too much of a skill issue to fix it, we can set this number to 1.
        .buffer_unordered(10)
        .for_each(|()| async {})
        .await;

    Ok(())
}

/// Initialize the client
///
/// # Errors
///
/// If the client cannot be initialized, an error is returned.
pub async fn init_client(rpc_port: u16) -> anyhow::Result<MusicPlayerClient> {
    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), rpc_port);

    let mut transport = tarpc::serde_transport::tcp::connect(server_addr, Bincode::default);
    transport.config_mut().max_frame_length(usize::MAX);

    // MusicPlayerClient is generated by the service attribute. It has a constructor `new` that takes a
    // config and any Transport as input.
    Ok(MusicPlayerClient::new(client::Config::default(), transport.await?).spawn())
}
