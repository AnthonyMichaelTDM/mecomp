//! The library state store.
//!
//! Updates every minute, or when the user requests a rescan, ands/removes/updates a playlist, or reclusters collections.

use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use mecomp_core::{
    errors::SerializableLibraryError, rpc::MusicPlayerClient, state::library::LibraryBrief,
};
use mecomp_storage::db::schemas;

use crate::termination::Interrupted;

use super::action::LibraryAction;

/// The library state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct LibraryState {
    state_tx: UnboundedSender<LibraryBrief>,
}

impl LibraryState {
    /// create a new library state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, UnboundedReceiver<LibraryBrief>) {
        let (state_tx, state_rx) = unbounded_channel::<LibraryBrief>();

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

        loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    handle_action(&mut state, &self.state_tx, daemon.clone(),action).await?;
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break Ok(interrupted);
                }
            }
        }
    }
}
async fn handle_action(
    state: &mut LibraryBrief,
    state_tx: &UnboundedSender<LibraryBrief>,
    daemon: Arc<MusicPlayerClient>,
    action: LibraryAction,
) -> anyhow::Result<()> {
    let mut update = false;
    let mut flag_update = || update = true;
    let current_context = tarpc::context::current;

    match action {
        LibraryAction::Rescan => rescan_library(daemon.clone()).await?,
        LibraryAction::Update => flag_update(),
        LibraryAction::Analyze => analyze_library(daemon.clone()).await?,
        LibraryAction::Recluster => recluster_library(daemon.clone()).await?,
        LibraryAction::CreatePlaylist(name) => daemon
            .playlist_get_or_create(current_context(), name)
            .await?
            .map(|_| flag_update())?,
        LibraryAction::RemovePlaylist(id) => {
            assert_eq!(id.tb, schemas::playlist::TABLE_NAME);
            daemon
                .playlist_remove(current_context(), id)
                .await?
                .map(|()| flag_update())?;
        }
        LibraryAction::RenamePlaylist(id, name) => {
            assert_eq!(id.tb, schemas::playlist::TABLE_NAME);
            daemon
                .playlist_rename(current_context(), id, name)
                .await?
                .map(|_| flag_update())?;
        }
        LibraryAction::RemoveSongsFromPlaylist(playlist, songs) => {
            assert_eq!(playlist.tb, schemas::playlist::TABLE_NAME);
            assert!(songs.iter().all(|s| s.tb == schemas::song::TABLE_NAME));
            daemon
                .playlist_remove_songs(current_context(), playlist, songs)
                .await??;
        }
        LibraryAction::AddThingsToPlaylist(playlist, things) => {
            assert_eq!(playlist.tb, schemas::playlist::TABLE_NAME);
            daemon
                .playlist_add_list(current_context(), playlist, things)
                .await??;
        }
        LibraryAction::CreatePlaylistAndAddThings(name, things) => {
            let playlist = daemon
                .playlist_get_or_create(current_context(), name)
                .await??;
            daemon
                .playlist_add_list(current_context(), playlist, things)
                .await?
                .map(|()| flag_update())?;
        }
        LibraryAction::CreateDynamicPlaylist(name, query) => daemon
            .dynamic_playlist_create(current_context(), name, query)
            .await?
            .map(|_| flag_update())?,
        LibraryAction::RemoveDynamicPlaylist(id) => {
            assert_eq!(id.tb, schemas::dynamic::TABLE_NAME);
            daemon
                .dynamic_playlist_remove(current_context(), id)
                .await?
                .map(|()| flag_update())?;
        }
        LibraryAction::UpdateDynamicPlaylist(id, changes) => {
            assert_eq!(id.tb, schemas::dynamic::TABLE_NAME);
            daemon
                .dynamic_playlist_update(current_context(), id, changes)
                .await?
                .map(|_| flag_update())?;
        }
    }

    if update {
        *state = get_library(daemon).await?;
        state_tx.send(state.clone())?;
    }

    Ok(())
}

async fn get_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<LibraryBrief> {
    let ctx = tarpc::context::current();
    Ok(daemon.library_brief(ctx).await??)
}

/// initiate a rescan and wait until it's done
async fn rescan_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    // don't error out is a rescan is in progress
    match daemon.library_rescan(ctx).await? {
        Ok(()) | Err(SerializableLibraryError::RescanInProgress) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// initiate an analysis and wait until it's done
async fn analyze_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    // don't error out if an analysis is in progress
    match daemon.library_analyze(ctx, false).await? {
        Ok(()) | Err(SerializableLibraryError::AnalysisInProgress) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// initiate a recluster and wait until it's done
async fn recluster_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    match daemon.library_recluster(ctx).await? {
        Ok(()) | Err(SerializableLibraryError::ReclusterInProgress) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
