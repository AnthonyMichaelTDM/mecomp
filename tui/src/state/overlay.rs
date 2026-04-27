//! This module implements the overlay state store.
//! Which handles opening and closing overlays.
//!
//! Overlays are different from popups in that they are rendered above popups and can be updated independently.
//! And that they can have a side-effect when they are closed (for example, change the selected option in a dropdown when it is closed).
//!
//! They are used for things like dropdowns, and are rendered on top of the popups.

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::{
    state::action::OverlayAction, termination::Interrupted, ui::widgets::overlay::OverlayType,
};

/// The overlay state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct OverlayState {
    state_tx: UnboundedSender<Option<OverlayType>>,
}

#[allow(clippy::module_name_repetitions)]
pub type OverlayStateReceiver = UnboundedReceiver<Option<OverlayType>>;

impl OverlayState {
    /// create a new overlay state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, OverlayStateReceiver) {
        let (state_tx, state_rx) = unbounded_channel::<Option<OverlayType>>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the overlay state every tick.
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    pub async fn main_loop(
        &self,
        mut action_rx: UnboundedReceiver<OverlayAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // the initial state once
        self.state_tx.send(None)?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    match action {
                        OverlayAction::Open(overlay) => {
                            self.state_tx.send(Some(overlay))?;
                        }
                        OverlayAction::Close => {
                            self.state_tx.send(None)?;
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
