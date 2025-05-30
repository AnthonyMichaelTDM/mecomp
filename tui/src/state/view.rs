//! The `ViewStore` is responsible for managing the `CurrentView` to be displayed.

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
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
        let mut view_history = vec![state.clone()];
        let mut view_index = 0;

        // the initial state once
        self.state_tx.send(state.clone())?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    state = self.handle_action(&mut view_history, &mut view_index, action);
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
        view_history: &mut Vec<ActiveView>,
        view_index: &mut usize,
        action: ViewAction,
    ) -> ActiveView {
        match action {
            ViewAction::Set(view) => {
                view_history.truncate(*view_index + 1);
                view_history.push(view);
                *view_index = view_history.len() - 1;
            }
            ViewAction::Back if *view_index > 0 => *view_index -= 1,
            ViewAction::Next if *view_index < view_history.len() - 1 => *view_index += 1,
            _ => {}
        }
        view_history.get(*view_index).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_handle_action() {
        let (view, _) = ViewState::new();

        let mut view_history = vec![ActiveView::default()];
        let mut view_index = 0;

        let mut state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Search),
        );
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Songs),
        );
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Artists),
        );
        assert_eq!(state, ActiveView::Artists);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::default());

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::default());
    }

    #[test]
    fn test_forward_backward() {
        let (view, _) = ViewState::new();

        let mut view_history = vec![ActiveView::default()];
        let mut view_index = 0;

        let mut state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Search),
        );
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Songs),
        );
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Artists),
        );
        assert_eq!(state, ActiveView::Artists);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(
            &mut view_history,
            &mut view_index,
            ViewAction::Set(ActiveView::Albums),
        );
        assert_eq!(state, ActiveView::Albums);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Next);
        assert_eq!(state, ActiveView::Albums);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Next);
        assert_eq!(state, ActiveView::Albums);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Songs);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::Search);

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::default());

        state = view.handle_action(&mut view_history, &mut view_index, ViewAction::Back);
        assert_eq!(state, ActiveView::default());
    }
}
