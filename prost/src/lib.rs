// we can't really control the code-gen, so we have to allow some lints here
#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate
)]

mod mecomp {
    include!("../out/mecomp.rs");
}

use std::time::Duration;

pub use mecomp::*;

use crate::mecomp::music_player_client::MusicPlayerClient;
use tonic::transport::Channel;

#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    #[error("Transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("Failed to connect to Music Player Daemon on port {port} after {retries} retries")]
    MaxRetriesExceeded { port: u16, retries: u64 },
}

/// Initialize the music player client
///
/// # Errors
///
/// If the client cannot be initialized, an error is returned.
///
/// # Panics
///
/// Panics if <https://localhost:{rpc_port}> is not a valid URL.
pub async fn init_client(rpc_port: u16) -> Result<MusicPlayerClient<Channel>, ConnectionError> {
    let endpoint = format!("http://localhost:{rpc_port}");

    let endpoint = Channel::from_shared(endpoint)
        .expect("Invalid endpoint URL")
        .connect()
        .await?;
    Ok(MusicPlayerClient::new(endpoint))
}

/// Initialize a client to the Music Player Daemon, with `MAX_RETRIES` retries spaced `DELAY` seconds apart
///
/// Will log intermediate failures as warnings.
///
/// # Errors
///
/// Fails if the maximum number of retries was exceeded
#[allow(clippy::missing_inline_in_public_items)]
pub async fn init_client_with_retry<const MAX_RETRIES: u64, const DELAY: u64>(
    rpc_port: u16,
) -> Result<MusicPlayerClient<Channel>, ConnectionError> {
    let mut retries = 0u64;

    while retries < MAX_RETRIES {
        match init_client(rpc_port).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                retries += 1;
                log::warn!("Failed to connect to daemon: {e}");
                tokio::time::sleep(Duration::from_secs(DELAY * retries)).await;
            }
        }
    }

    log::error!("{MAX_RETRIES} retries exceeded when attempting to connect to the daemon");

    Err(ConnectionError::MaxRetriesExceeded {
        port: rpc_port,
        retries: MAX_RETRIES,
    })
}
