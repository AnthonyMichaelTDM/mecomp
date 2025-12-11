//! The library state store.
//!
//! Updates every minute, or when the user requests a rescan, ands/removes/updates a playlist, or reclusters collections.

use mecomp_prost::{
    DynamicPlaylistCreateRequest, DynamicPlaylistUpdateRequest, LibraryAnalyzeRequest,
    LibraryBriefResponse as LibraryBrief, MusicPlayerClient, PlaylistAddListRequest, PlaylistName,
    PlaylistRemoveSongsRequest, PlaylistRenameRequest,
};
use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

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
        mut daemon: MusicPlayerClient,
        mut action_rx: UnboundedReceiver<LibraryAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // the initial state once
        let state = get_library(&mut daemon).await?;
        self.state_tx.send(state)?;

        loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    handle_action(&self.state_tx, &mut daemon, action).await?;
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
    state_tx: &UnboundedSender<LibraryBrief>,
    daemon: &mut MusicPlayerClient,
    action: LibraryAction,
) -> anyhow::Result<()> {
    let mut update = false;
    let mut flag_update = || update = true;

    match action {
        LibraryAction::Rescan => rescan_library(daemon).await?,
        LibraryAction::Update => flag_update(),
        LibraryAction::Analyze => analyze_library(daemon).await?,
        LibraryAction::Recluster => recluster_library(daemon).await?,
        LibraryAction::CreatePlaylist(name) => daemon
            .playlist_get_or_create(PlaylistName::new(name))
            .await?
            .map(|_| flag_update())
            .into_inner(),
        LibraryAction::RemovePlaylist(id) => daemon
            .playlist_remove(id)
            .await?
            .map(|()| flag_update())
            .into_inner(),
        LibraryAction::RenamePlaylist(id, name) => daemon
            .playlist_rename(PlaylistRenameRequest::new(id, name))
            .await?
            .map(|_| flag_update())
            .into_inner(),
        LibraryAction::RemoveSongsFromPlaylist(playlist, songs) => daemon
            .playlist_remove_songs(PlaylistRemoveSongsRequest::new(playlist, songs))
            .await?
            .map(|()| flag_update())
            .into_inner(),
        LibraryAction::AddThingsToPlaylist(playlist, things) => daemon
            .playlist_add_list(PlaylistAddListRequest::new(playlist, things))
            .await?
            .map(|()| flag_update())
            .into_inner(),
        LibraryAction::CreatePlaylistAndAddThings(name, things) => {
            let playlist = daemon
                .playlist_get_or_create(PlaylistName::new(name))
                .await?
                .into_inner();
            daemon
                .playlist_add_list(PlaylistAddListRequest::new(playlist, things))
                .await?
                .map(|()| flag_update())
                .into_inner();
        }
        LibraryAction::CreateDynamicPlaylist(name, query) => daemon
            .dynamic_playlist_create(DynamicPlaylistCreateRequest::new(name, query))
            .await?
            .map(|_| flag_update())
            .into_inner(),
        LibraryAction::RemoveDynamicPlaylist(id) => daemon
            .dynamic_playlist_remove(id)
            .await?
            .map(|()| flag_update())
            .into_inner(),
        LibraryAction::UpdateDynamicPlaylist(id, changes) => daemon
            .dynamic_playlist_update(DynamicPlaylistUpdateRequest::new(id, changes))
            .await?
            .map(|_| flag_update())
            .into_inner(),
    }

    if update {
        let state = get_library(daemon).await?;
        state_tx.send(state)?;
    }

    Ok(())
}

async fn get_library(daemon: &mut MusicPlayerClient) -> anyhow::Result<LibraryBrief> {
    Ok(daemon.library_brief(()).await?.into_inner())
}

/// initiate a rescan and wait until it's done
async fn rescan_library(daemon: &mut MusicPlayerClient) -> anyhow::Result<()> {
    // don't error out is a rescan is in progress
    match daemon.library_rescan(()).await {
        Ok(_) => Ok(()),
        Err(e) if e.code() == tonic::Code::Aborted => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// initiate an analysis and wait until it's done
async fn analyze_library(daemon: &mut MusicPlayerClient) -> anyhow::Result<()> {
    // don't error out if an analysis is in progress
    match daemon
        .library_analyze(LibraryAnalyzeRequest::new(false))
        .await
    {
        Ok(_) => Ok(()),
        Err(e) if e.code() == tonic::Code::Aborted => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// initiate a recluster and wait until it's done
async fn recluster_library(daemon: &mut MusicPlayerClient) -> anyhow::Result<()> {
    match daemon.library_recluster(()).await {
        Ok(_) => Ok(()),
        Err(e) if e.code() == tonic::Code::Aborted => Ok(()),
        Err(e) => Err(e.into()),
    }
}
