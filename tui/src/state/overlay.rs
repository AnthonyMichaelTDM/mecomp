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
    state::action::OverlayAction,
    termination::Interrupted,
    ui::widgets::overlay::{OverlayResult, OverlayType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayUpdate {
    pub active: Option<OverlayType>,
    pub result: Option<OverlayResult>,
}

impl OverlayUpdate {
    #[must_use]
    pub const fn new(active: Option<OverlayType>, result: Option<OverlayResult>) -> Self {
        Self { active, result }
    }
}

/// The overlay state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct OverlayState {
    state_tx: UnboundedSender<OverlayUpdate>,
}

#[allow(clippy::module_name_repetitions)]
pub type OverlayStateReceiver = UnboundedReceiver<OverlayUpdate>;

impl OverlayState {
    /// create a new overlay state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, OverlayStateReceiver) {
        let (state_tx, state_rx) = unbounded_channel::<OverlayUpdate>();

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
        self.state_tx.send(OverlayUpdate::new(None, None))?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    match action {
                        OverlayAction::Open(overlay) => {
                            self.state_tx.send(OverlayUpdate::new(Some(overlay), None))?;
                        }
                        OverlayAction::Commit(result) => {
                            self.state_tx.send(OverlayUpdate::new(None, Some(result)))?;
                        }
                        OverlayAction::Close => {
                            self.state_tx.send(OverlayUpdate::new(None, None))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::overlay::OverlayResult;

    #[tokio::test]
    async fn commit_emits_result_and_closes_overlay() {
        let (store, mut state_rx) = OverlayState::new();
        let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
        let (interrupt_tx, interrupt_rx) = tokio::sync::broadcast::channel(1);

        let join = tokio::spawn(async move { store.main_loop(action_rx, interrupt_rx).await });

        let initial = state_rx.recv().await.expect("initial overlay state");
        assert_eq!(initial, OverlayUpdate::new(None, None));

        action_tx
            .send(OverlayAction::Commit(OverlayResult::DropdownSelected {
                target_id: 42,
                selected_index: 2,
            }))
            .expect("send commit action");

        let update = state_rx.recv().await.expect("commit overlay update");
        assert_eq!(
            update,
            OverlayUpdate::new(
                None,
                Some(OverlayResult::DropdownSelected {
                    target_id: 42,
                    selected_index: 2,
                })
            )
        );

        interrupt_tx
            .send(Interrupted::UserInt)
            .expect("interrupt sent");
        let result = join
            .await
            .expect("join overlay task")
            .expect("overlay loop ok");
        assert_eq!(result, Interrupted::UserInt);
    }
}
