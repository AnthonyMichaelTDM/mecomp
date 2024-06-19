//! The `ViewStore` is responsible for managing the `CurrentView` to be displayed.

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use crate::{termination::Interrupted, ui::components::content_view::ActiveView};

/// The `ViewStore` is responsible for managing the `CurrentView` to be displayed.
#[allow(clippy::module_name_repetitions)]
pub struct ViewStore {
    state_tx: UnboundedSender<ActiveView>,
}

impl ViewStore {
    /// Create a new `ViewStore`.
    pub fn new() -> (Self, UnboundedReceiver<ActiveView>) {
        let (state_tx, state_rx) = unbounded_channel::<ActiveView>();
        (Self { state_tx }, state_rx)
    }

    /// A loop that updates the store when requested
    pub async fn main_loop(
        &self,
        mut action_rx: UnboundedReceiver<ActiveView>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = ActiveView::default();

        // the initial state once
        self.state_tx.send(state)?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    state = action;
                    self.state_tx.send(state)?;
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
