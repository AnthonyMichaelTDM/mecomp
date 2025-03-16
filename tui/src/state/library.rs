//! The library state store.
//!
//! Updates every minute, or when the user requests a rescan, ands/removes/updates a playlist, or reclusters collections.

use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use mecomp_core::{rpc::MusicPlayerClient, state::library::LibraryFull};

use crate::termination::Interrupted;

use super::action::LibraryAction;

/// The library state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct LibraryState {
    state_tx: UnboundedSender<LibraryFull>,
}

impl LibraryState {
    /// create a new library state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, UnboundedReceiver<LibraryFull>) {
        let (state_tx, state_rx) = unbounded_channel::<LibraryFull>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the library state every tick.
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    /// or if the daemon client can't connect to the server
    /// or if the daemon returns an error
    pub async fn main_loop(
        &self,
        daemon: Arc<MusicPlayerClient>,
        mut action_rx: UnboundedReceiver<LibraryAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = get_library(daemon.clone()).await?;

        // the initial state once
        self.state_tx.send(state.clone())?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    match action {
                        LibraryAction::Rescan => {
                            rescan_library(daemon.clone()).await?;
                        }
                        LibraryAction::Update => {
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::Analyze => {
                            analyze_library(daemon.clone()).await?;
                        }
                        LibraryAction::Recluster => {
                            recluster_library(daemon.clone()).await?;
                        }
                        LibraryAction::CreatePlaylist(name) => {
                            daemon.playlist_get_or_create(tarpc::context::current(), name).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::RemovePlaylist(id) => {
                            debug_assert_eq!(
                                id.tb,
                                mecomp_storage::db::schemas::playlist::TABLE_NAME
                            );
                            daemon.playlist_remove(tarpc::context::current(), id).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::RenamePlaylist(id, name) => {
                            debug_assert_eq!(
                                id.tb,
                                mecomp_storage::db::schemas::playlist::TABLE_NAME
                            );
                            daemon.playlist_rename(tarpc::context::current(), id, name).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::RemoveSongsFromPlaylist(playlist, songs) => {
                            debug_assert_eq!(
                                playlist.tb,
                                mecomp_storage::db::schemas::playlist::TABLE_NAME
                            );
                            debug_assert!(songs.iter().all(|s| s.tb == mecomp_storage::db::schemas::song::TABLE_NAME));
                            daemon.playlist_remove_songs(tarpc::context::current(), playlist, songs).await??;
                        }
                        LibraryAction::AddThingsToPlaylist(playlist, things) => {
                            debug_assert_eq!(
                                playlist.tb,
                                mecomp_storage::db::schemas::playlist::TABLE_NAME
                            );
                            daemon.playlist_add_list(tarpc::context::current(), playlist, things).await??;
                        }
                        LibraryAction::CreatePlaylistAndAddThings(name, things) => {
                            let playlist = daemon.playlist_get_or_create(tarpc::context::current(), name).await??;
                            daemon.playlist_add_list(tarpc::context::current(), playlist, things).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::CreateDynamicPlaylist(name, query) => {
                            daemon.dynamic_playlist_create(tarpc::context::current(), name, query).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::RemoveDynamicPlaylist(id) => {
                            debug_assert_eq!(
                                id.tb,
                                mecomp_storage::db::schemas::dynamic::TABLE_NAME
                            );
                            daemon.dynamic_playlist_remove(tarpc::context::current(), id).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::UpdateDynamicPlaylist(id, changes) => {
                            debug_assert_eq!(
                                id.tb,
                                mecomp_storage::db::schemas::dynamic::TABLE_NAME
                            );
                            daemon.dynamic_playlist_update(tarpc::context::current(), id, changes).await??;
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                    }
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }
        };

        Ok(result)
    }
}

async fn get_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<LibraryFull> {
    let ctx = tarpc::context::current();
    Ok(daemon.library_full(ctx).await??)
}

/// initiate a rescan and wait until it's done
async fn rescan_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    daemon.library_rescan(ctx).await??;

    Ok(())
}

/// initiate an analysis and wait until it's done
async fn analyze_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    daemon.library_analyze(ctx, false).await??;

    Ok(())
}

/// initiate a recluster and wait until it's done
async fn recluster_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    daemon.library_recluster(ctx).await??;

    Ok(())
}
