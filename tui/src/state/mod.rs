use action::Action;

use mecomp_core::state::StateAudio;
use mecomp_prost::{LibraryBrief, MusicPlayerClient, SearchResult};
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};

use crate::{
    termination::{Interrupted, Terminator},
    ui::{components::content_view::ActiveView, widgets::popups::PopupType},
};

pub mod action;
pub mod audio;
pub mod component;
pub mod library;
pub mod overlay;
pub mod popup;
pub mod search;
pub mod view;

/// an all-in-one dispactcher for managing state updates.
pub struct Dispatcher {
    audio: audio::AudioState,
    search: search::SearchState,
    library: library::LibraryState,
    view: view::ViewState,
    overlay: overlay::OverlayState,
    popup: popup::PopupState,
    component: component::ComponentState,
}

/// a struct that centralized the senders for all the state stores.
struct Senders {
    pub audio: UnboundedSender<action::AudioAction>,
    pub search: UnboundedSender<String>,
    pub library: UnboundedSender<action::LibraryAction>,
    pub view: UnboundedSender<action::ViewAction>,
    pub overlay: UnboundedSender<action::OverlayAction>,
    pub popup: UnboundedSender<action::PopupAction>,
    pub component: UnboundedSender<action::ComponentAction>,
}

/// a struct that centralized the receivers for all the state stores.
pub struct Receivers {
    pub audio: UnboundedReceiver<StateAudio>,
    pub search: UnboundedReceiver<SearchResult>,
    pub library: UnboundedReceiver<LibraryBrief>,
    pub view: UnboundedReceiver<ActiveView>,
    pub overlay: UnboundedReceiver<overlay::OverlayUpdate>,
    pub popup: UnboundedReceiver<Option<PopupType>>,
    pub component: UnboundedReceiver<component::ActiveComponent>,
}

impl Dispatcher {
    #[must_use]
    pub fn new() -> (Self, Receivers) {
        let (audio, audio_rx) = audio::AudioState::new();
        let (search, search_rx) = search::SearchState::new();
        let (library, library_rx) = library::LibraryState::new();
        let (view, view_rx) = view::ViewState::new();
        let (overlay, overlay_rx) = overlay::OverlayState::new();
        let (popup, popup_rx) = popup::PopupState::new();
        let (active_component, active_component_rx) = component::ComponentState::new();

        let dispatcher = Self {
            audio,
            search,
            library,
            view,
            overlay,
            popup,
            component: active_component,
        };
        let state_receivers = Receivers {
            audio: audio_rx,
            search: search_rx,
            library: library_rx,
            view: view_rx,
            overlay: overlay_rx,
            popup: popup_rx,
            component: active_component_rx,
        };

        (dispatcher, state_receivers)
    }

    /// the main loop for the dispatcher.
    ///
    /// the dispatcher will run until the user exits the application.
    ///
    /// # Errors
    ///
    /// if any of the state stores fail to run.
    pub async fn main_loop(
        &self,
        daemon: MusicPlayerClient,
        terminator: Terminator,
        action_rx: UnboundedReceiver<Action>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let (audio_action_tx, audio_action_rx) = mpsc::unbounded_channel();
        let (search_action_tx, search_action_rx) = mpsc::unbounded_channel();
        let (library_action_tx, library_action_rx) = mpsc::unbounded_channel();
        let (view_action_tx, view_action_rx) = mpsc::unbounded_channel();
        let (overlay_action_tx, overlay_action_rx) = mpsc::unbounded_channel();
        let (popup_action_tx, popup_action_rx) = mpsc::unbounded_channel();
        let (component_action_tx, component_action_rx) = mpsc::unbounded_channel();

        // run multiple tasks in parallel, and wait for all of them to finish.
        // the tasks are:
        // - the audio state store
        // - ...
        // - the action dispatcher
        tokio::try_join!(
            // the audio state store
            self.audio
                .main_loop(daemon.clone(), audio_action_rx, interrupt_rx.resubscribe()),
            // the search state store
            self.search
                .main_loop(daemon.clone(), search_action_rx, interrupt_rx.resubscribe()),
            // the library state store
            self.library.main_loop(
                daemon.clone(),
                library_action_rx,
                interrupt_rx.resubscribe()
            ),
            // the view store
            self.view
                .main_loop(view_action_rx, interrupt_rx.resubscribe()),
            // the overlay store
            self.overlay
                .main_loop(overlay_action_rx, interrupt_rx.resubscribe()),
            // the popup store
            self.popup
                .main_loop(popup_action_rx, interrupt_rx.resubscribe()),
            // the active component store
            self.component
                .main_loop(component_action_rx, interrupt_rx.resubscribe()),
            // the action dispatcher
            Self::action_dispatcher(
                terminator,
                action_rx,
                Senders {
                    audio: audio_action_tx,
                    search: search_action_tx,
                    library: library_action_tx,
                    view: view_action_tx,
                    overlay: overlay_action_tx,
                    popup: popup_action_tx,
                    component: component_action_tx,
                },
            ),
        )?;

        Ok(interrupt_rx.recv().await?)
    }

    async fn action_dispatcher(
        mut terminator: Terminator,
        mut action_rx: UnboundedReceiver<Action>,
        senders: Senders,
    ) -> anyhow::Result<()> {
        while let Some(action) = action_rx.recv().await {
            match action {
                Action::Audio(action) => {
                    senders.audio.send(action)?;
                }
                Action::Search(query) => {
                    senders.search.send(query)?;
                }
                Action::General(action) => match action {
                    action::GeneralAction::Exit => {
                        let _ = terminator.terminate(Interrupted::UserInt);

                        break;
                    }
                },
                Action::Library(action) => {
                    senders.library.send(action)?;
                }
                Action::ActiveView(action) => {
                    senders.view.send(action)?;
                }
                Action::Overlay(overlay) => {
                    senders.overlay.send(overlay)?;
                }
                Action::Popup(popup) => {
                    if popup == action::PopupAction::Close {
                        senders.overlay.send(action::OverlayAction::Close)?;
                    }
                    senders.popup.send(popup)?;
                }
                Action::ActiveComponent(action) => {
                    senders.component.send(action)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::action::{
        Action, AudioAction, ComponentAction, GeneralAction, LibraryAction, OverlayAction,
        PlaybackAction, PopupAction, ViewAction,
    };
    use crate::state::component::ActiveComponent;
    use crate::termination::create_termination;
    use crate::ui::components::content_view::ActiveView;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    async fn test_action_dispatcher_forwards_actions() {
        let (terminator, _) = create_termination();
        let (action_tx, action_rx) = unbounded_channel::<Action>();
        let (audio_tx, mut audio_rx) = unbounded_channel::<AudioAction>();
        let (search_tx, mut search_rx) = unbounded_channel::<String>();
        let (library_tx, mut library_rx) = unbounded_channel::<LibraryAction>();
        let (view_tx, mut view_rx) = unbounded_channel::<ViewAction>();
        let (overlay_tx, mut overlay_rx) = unbounded_channel::<OverlayAction>();
        let (popup_tx, mut popup_rx) = unbounded_channel::<PopupAction>();
        let (component_tx, mut component_rx) = unbounded_channel::<ComponentAction>();

        let senders = Senders {
            audio: audio_tx,
            search: search_tx,
            library: library_tx,
            view: view_tx,
            overlay: overlay_tx,
            popup: popup_tx,
            component: component_tx,
        };

        let dispatcher_handle = tokio::spawn(async move {
            Dispatcher::action_dispatcher(terminator, action_rx, senders).await
        });

        action_tx
            .send(Action::Audio(AudioAction::Playback(PlaybackAction::Toggle)))
            .unwrap();
        action_tx.send(Action::Search("query".into())).unwrap();
        action_tx
            .send(Action::Library(LibraryAction::Update))
            .unwrap();
        action_tx
            .send(Action::ActiveView(ViewAction::Set(ActiveView::Search)))
            .unwrap();
        action_tx.send(Action::Popup(PopupAction::Close)).unwrap();
        action_tx
            .send(Action::ActiveComponent(ComponentAction::Set(
                ActiveComponent::ContentView,
            )))
            .unwrap();
        action_tx
            .send(Action::General(GeneralAction::Exit))
            .unwrap();

        let result = dispatcher_handle.await.unwrap();
        assert!(result.is_ok());

        assert_eq!(
            audio_rx.recv().await.unwrap(),
            AudioAction::Playback(PlaybackAction::Toggle)
        );
        assert_eq!(search_rx.recv().await.unwrap(), "query");
        assert_eq!(library_rx.recv().await.unwrap(), LibraryAction::Update);
        assert_eq!(
            view_rx.recv().await.unwrap(),
            ViewAction::Set(ActiveView::Search)
        );
        assert_eq!(overlay_rx.recv().await.unwrap(), OverlayAction::Close);
        assert_eq!(popup_rx.recv().await.unwrap(), PopupAction::Close);
        assert_eq!(
            component_rx.recv().await.unwrap(),
            ComponentAction::Set(ActiveComponent::ContentView)
        );
    }

    #[test]
    fn test_dispatcher_new_initializes_receivers() {
        let (_dispatcher, mut receivers) = Dispatcher::new();

        assert!(receivers.audio.try_recv().is_err());
        assert!(receivers.search.try_recv().is_err());
        assert!(receivers.library.try_recv().is_err());
        assert!(receivers.view.try_recv().is_err());
        assert!(receivers.overlay.try_recv().is_err());
        assert!(receivers.popup.try_recv().is_err());
        assert!(receivers.component.try_recv().is_err());
    }
}
