//! This module implements the popup state store.
//! Which handles opening and closing popups.

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use crate::{state::action::PopupAction, termination::Interrupted, ui::widgets::popups::PopupType};

/// The popup state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct PopupState {
    state_tx: UnboundedSender<Option<PopupType>>,
}

#[allow(clippy::module_name_repetitions)]
pub type PopupStateReceiver = UnboundedReceiver<Option<PopupType>>;

impl PopupState {
    /// create a new popup state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, PopupStateReceiver) {
        let (state_tx, state_rx) = unbounded_channel::<Option<PopupType>>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the popup state every tick.
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    pub async fn main_loop(
        &self,
        mut action_rx: UnboundedReceiver<PopupAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // the initial state once
        self.state_tx.send(None)?;

        // popup stack
        let mut stack = Vec::new();

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    match action {
                        PopupAction::Open(popup) => {
                            stack.push(popup.clone());
                            self.state_tx.send(Some(popup))?;
                        }
                        PopupAction::Close => {
                            self.state_tx.send(stack.pop())?;
                        }
                    }
                }
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }
        };

        Ok(result)
    }
}
