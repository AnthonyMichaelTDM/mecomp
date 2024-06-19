//! The library state store.
//!
//! Updates every minute, or when the user requests a rescan, ands/removes/updates a playlist, or reclusters collections.

use std::{sync::Arc, time::Duration};

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
    pub fn new() -> (Self, UnboundedReceiver<LibraryFull>) {
        let (state_tx, state_rx) = unbounded_channel::<LibraryFull>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the library state every tick.
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
                            state = rescan_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::Update => {
                            state = get_library(daemon.clone()).await?;
                            self.state_tx.send(state.clone())?;
                        }
                        LibraryAction::Analyze => {
                            analyze_library(daemon.clone()).await?;
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
async fn rescan_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<LibraryFull> {
    let ctx = tarpc::context::current();

    daemon.library_rescan(ctx).await??;

    // wait for it to finish
    while daemon
        .library_rescan_in_progress(tarpc::context::current())
        .await?
    {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // return the new library
    let ctx = tarpc::context::current();
    Ok(daemon.library_full(ctx).await??)
}

/// initiate an analysis and wait until it's done
async fn analyze_library(daemon: Arc<MusicPlayerClient>) -> anyhow::Result<()> {
    let ctx = tarpc::context::current();

    daemon.library_analyze(ctx).await??;

    // wait for it to finish
    while daemon
        .library_analyze_in_progress(tarpc::context::current())
        .await?
    {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
