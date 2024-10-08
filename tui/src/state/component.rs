use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender},
};

use crate::termination::Interrupted;

use super::action::ComponentAction;

// The audio state store.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct ComponentState {
    state_tx: UnboundedSender<ActiveComponent>,
}

impl ComponentState {
    /// create a new audio state store, and return the receiver for listening to state updates.
    #[must_use]
    pub fn new() -> (Self, UnboundedReceiver<ActiveComponent>) {
        let (state_tx, state_rx) = tokio::sync::mpsc::unbounded_channel::<ActiveComponent>();

        (Self { state_tx }, state_rx)
    }

    /// a loop that updates the active component when requested
    ///
    /// # Errors
    ///
    /// Fails if the state cannot be sent
    pub async fn main_loop(
        &self,
        mut action_rx: UnboundedReceiver<ComponentAction>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut state = ActiveComponent::default();

        // the initial state once
        self.state_tx.send(state)?;

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                Some(action) = action_rx.recv() => {
                    state = Self::handle_action(state, action);
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

    /// Handles the action, returning the new state.
    #[must_use]
    const fn handle_action(state: ActiveComponent, action: ComponentAction) -> ActiveComponent {
        match action {
            ComponentAction::Next => state.next(),
            ComponentAction::Previous => state.prev(),
            ComponentAction::Set(new_state) => new_state,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::module_name_repetitions)]
pub enum ActiveComponent {
    #[default]
    Sidebar,
    QueueBar,
    ControlPanel,
    ContentView,
}

impl ActiveComponent {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::ContentView,
            Self::ContentView => Self::QueueBar,
            Self::QueueBar => Self::ControlPanel,
            Self::ControlPanel => Self::Sidebar,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Sidebar => Self::ControlPanel,
            Self::ContentView => Self::Sidebar,
            Self::QueueBar => Self::ContentView,
            Self::ControlPanel => Self::QueueBar,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_next() {
        assert_eq!(
            ActiveComponent::Sidebar.next(),
            ActiveComponent::ContentView
        );
        assert_eq!(
            ActiveComponent::ContentView.next(),
            ActiveComponent::QueueBar
        );
        assert_eq!(
            ActiveComponent::QueueBar.next(),
            ActiveComponent::ControlPanel
        );
        assert_eq!(
            ActiveComponent::ControlPanel.next(),
            ActiveComponent::Sidebar
        );
    }

    #[test]
    fn test_prev() {
        assert_eq!(
            ActiveComponent::Sidebar.prev(),
            ActiveComponent::ControlPanel
        );
        assert_eq!(
            ActiveComponent::ContentView.prev(),
            ActiveComponent::Sidebar
        );
        assert_eq!(
            ActiveComponent::QueueBar.prev(),
            ActiveComponent::ContentView
        );
        assert_eq!(
            ActiveComponent::ControlPanel.prev(),
            ActiveComponent::QueueBar
        );
    }

    #[rstest]
    #[case::next(
        ActiveComponent::Sidebar,
        ComponentAction::Next,
        ActiveComponent::ContentView
    )]
    #[case::next(
        ActiveComponent::ContentView,
        ComponentAction::Next,
        ActiveComponent::QueueBar
    )]
    #[case::next(
        ActiveComponent::QueueBar,
        ComponentAction::Next,
        ActiveComponent::ControlPanel
    )]
    #[case::next(
        ActiveComponent::ControlPanel,
        ComponentAction::Next,
        ActiveComponent::Sidebar
    )]
    #[case::prev(
        ActiveComponent::Sidebar,
        ComponentAction::Previous,
        ActiveComponent::ControlPanel
    )]
    #[case::prev(
        ActiveComponent::ContentView,
        ComponentAction::Previous,
        ActiveComponent::Sidebar
    )]
    #[case::prev(
        ActiveComponent::QueueBar,
        ComponentAction::Previous,
        ActiveComponent::ContentView
    )]
    #[case::prev(
        ActiveComponent::ControlPanel,
        ComponentAction::Previous,
        ActiveComponent::QueueBar
    )]
    #[case::set(
        ActiveComponent::Sidebar,
        ComponentAction::Set(ActiveComponent::ContentView),
        ActiveComponent::ContentView
    )]
    #[case::set(
        ActiveComponent::ContentView,
        ComponentAction::Set(ActiveComponent::QueueBar),
        ActiveComponent::QueueBar
    )]
    fn test_handle_action(
        #[case] starting_state: ActiveComponent,
        #[case] action: ComponentAction,
        #[case] expected_state: ActiveComponent,
    ) {
        let new_state = ComponentState::handle_action(starting_state, action);

        assert_eq!(new_state, expected_state);
    }
}
