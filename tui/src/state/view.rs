//! The `ViewStore` is responsible for managing the `CurrentView` to be displayed.

use tokio::sync::{
    broadcast,
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use crate::{termination::Interrupted, ui::components::content_view::ActiveView};

use super::action::ViewAction;

/// The `ViewStore` is responsible for managing the `CurrentView` to be displayed.
#[allow(clippy::module_name_repetitions)]
pub struct ViewState {
    state_tx: UnboundedSender<ActiveView>,
}

impl ViewState {
    /// Create a new `ViewStore`.
    #[must_use]
    pub fn new() -> (Self, UnboundedReceiver<ActiveView>) {
        let (state_tx, state_rx) = unbounded_channel::<ActiveView>();
        (Self { state_tx }, state_rx)
    }

    /// A loop that updates the store when requested
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    pub async fn main_loop(
        &self,
        mut action_rx: UnboundedReceiver<ViewAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = ActiveView::default();
        // a stack to keep track of previous views
        let mut view_stack = Vec::new();

        // the initial state once
        self.state_tx.send(state.clone())?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    state = self.handle_action(&state, &mut view_stack, action);
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

    /// Handle the action, returning the new state
    pub fn handle_action(
        &self,
        state: &ActiveView,
        view_stack: &mut Vec<ActiveView>,
        action: ViewAction,
    ) -> ActiveView {
        match action {
            ViewAction::Set(view) => {
                view_stack.push(state.clone());
                view
            }
            ViewAction::Back => view_stack.pop().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_handle_action() {
        let (view, _) = ViewState::new();

        let mut view_stack = Vec::new();

        let mut state = view.handle_action(
            &ActiveView::default(),
            &mut view_stack,
            ViewAction::Set(ActiveView::Search),
        );
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(&state, &mut view_stack, ViewAction::Set(ActiveView::Songs));
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(
            &state,
            &mut view_stack,
            ViewAction::Set(ActiveView::Artists),
        );
        assert_eq!(state, ActiveView::Artists);

        state = view.handle_action(&state, &mut view_stack, ViewAction::Back);
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(&state, &mut view_stack, ViewAction::Back);
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(&state, &mut view_stack, ViewAction::Back);
        assert_eq!(state, ActiveView::default());

        state = view.handle_action(&state, &mut view_stack, ViewAction::Back);
        assert_eq!(state, ActiveView::default());
    }
}
