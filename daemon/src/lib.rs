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
use mecomp_storage::db::set_database_path;

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
}

pub mod config;
pub mod controller;
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
    settings: &DaemonSettings,
) -> anyhow::Result<()> {
    init_logger(log_level);
    set_database_path(settings.db_path.clone())?;
    let db = Arc::new(mecomp_storage::db::init_database().await?);
    tracing::subscriber::set_global_default(init_tracing())?;

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
        // serve is generated by the service attribute. It takes as input any type implementing
        // the generated MusicPlayer trait.
        .map(|channel| {
            let server =
                MusicPlayerServer::new(channel.transport().peer_addr().unwrap(), db.clone());
            channel.execute(server.serve()).for_each(spawn)
        })
        // Max 1 channels.
        // this means that we will only process one request at a time
        .buffer_unordered(1)
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
