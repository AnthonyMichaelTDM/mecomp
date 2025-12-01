mod mecomp {
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

    #[cfg(not(tarpaulin_include))]
    include!("../out/mecomp.rs");
}
#[doc(hidden)]
mod conversions;
pub mod helpers;

use std::time::Duration;

use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub use conversions::convert_std_duration;
pub use mecomp::music_player_client as client;
pub use mecomp::music_player_server as server;
pub use mecomp::*;
pub use tonic;

pub type LibraryBrief = mecomp::LibraryBriefResponse;
pub type LibraryFull = mecomp::LibraryFullResponse;
pub type LibraryHealth = mecomp::LibraryHealthResponse;

pub type MusicPlayerClient =
    client::MusicPlayerClient<InterceptedService<Channel, TraceInterceptor>>;

#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    #[error("{0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("failed to connect to Music Player Daemon on port {port} after {retries} retries")]
    MaxRetriesExceeded { port: u16, retries: u64 },
}

#[derive(Clone, Debug)]
pub struct TraceInterceptor {}
impl Interceptor for TraceInterceptor {
    fn call(&mut self, req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        tracing::trace!("Received request with extensions: {:?}", req.extensions());
        Ok(req)
    }
}

/// Initialize the music player client, without verifying the connection.
///
/// # Note
///
/// Does not check that the daemon is actually running, to get a verified connection use either `init_client` or `init_client_with_retry`
///
/// # Panics
///
/// Panics if <https://localhost:{rpc_port}> is not a valid URL.
#[must_use]
pub fn lazy_init_client(rpc_port: u16) -> MusicPlayerClient {
    let endpoint = format!("http://localhost:{rpc_port}");

    let endpoint = Channel::from_shared(endpoint)
        .expect("Invalid endpoint URL")
        .connect_lazy();

    let interceptor = TraceInterceptor {};

    music_player_client::MusicPlayerClient::with_interceptor(endpoint, interceptor)
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
pub async fn init_client(rpc_port: u16) -> Result<MusicPlayerClient, ConnectionError> {
    let endpoint = format!("http://localhost:{rpc_port}");

    let endpoint = Channel::from_shared(endpoint)
        .expect("Invalid endpoint URL")
        .connect()
        .await?;

    let interceptor = TraceInterceptor {};

    let client = music_player_client::MusicPlayerClient::with_interceptor(endpoint, interceptor);

    Ok(client)
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
) -> Result<MusicPlayerClient, ConnectionError> {
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
