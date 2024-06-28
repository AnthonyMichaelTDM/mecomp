//! This module contains the implementation of audio state store.
//! which is updated every tick and used by views to render the audio playback and queue state.
//!
//! The audio state store is responsible for maintaining the audio state, and for handling audio related actions.

use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use mecomp_core::rpc::{MusicPlayerClient, SearchResult};

use crate::termination::Interrupted;

/// The audio state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct SearchState {
    state_tx: UnboundedSender<SearchResult>,
}

impl SearchState {
    /// create a new audio state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, UnboundedReceiver<SearchResult>) {
        let (state_tx, state_rx) = unbounded_channel::<SearchResult>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the audio state every tick.
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    /// or if the daemon client can't connect to the server
    pub async fn main_loop(
        &self,
        daemon: Arc<MusicPlayerClient>,
        mut action_rx: UnboundedReceiver<String>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = SearchResult::default();

        // the initial state once
        self.state_tx.send(state.clone())?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(query) = action_rx.recv() => {
                    let ctx = tarpc::context::current();
                    state = daemon.search(ctx, query, 100).await?;
                    self.state_tx.send(state.clone())?;
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
