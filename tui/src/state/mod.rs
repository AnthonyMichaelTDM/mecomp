use std::sync::Arc;

use action::Action;
use mecomp_core::{
    rpc::{MusicPlayerClient, SearchResult},
    state::{library::LibraryFull, StateAudio},
};
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
pub mod popup;
pub mod search;
pub mod view;

/// an all-in-one dispactcher for managing state updates.
pub struct Dispatcher {
    audio: audio::AudioState,
    search: search::SearchState,
    library: library::LibraryState,
    view: view::ViewState,
    popup: popup::PopupState,
    component: component::ComponentState,
}

/// a struct that centralized the senders for all the state stores.
struct Senders {
    pub audio: UnboundedSender<action::AudioAction>,
    pub search: UnboundedSender<String>,
    pub library: UnboundedSender<action::LibraryAction>,
    pub view: UnboundedSender<action::ViewAction>,
    pub popup: UnboundedSender<action::PopupAction>,
    pub component: UnboundedSender<action::ComponentAction>,
}

/// a struct that centralized the receivers for all the state stores.
pub struct Receivers {
    pub audio: UnboundedReceiver<StateAudio>,
    pub search: UnboundedReceiver<SearchResult>,
    pub library: UnboundedReceiver<LibraryFull>,
    pub view: UnboundedReceiver<ActiveView>,
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
        let (popup, popup_rx) = popup::PopupState::new();
        let (active_component, active_component_rx) = component::ComponentState::new();

        let dispatcher = Self {
            audio,
            search,
            library,
            view,
            popup,
            component: active_component,
        };
        let state_receivers = Receivers {
            audio: audio_rx,
            search: search_rx,
            library: library_rx,
            view: view_rx,
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
        daemon: Arc<MusicPlayerClient>,
        terminator: Terminator,
        action_rx: UnboundedReceiver<Action>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let (audio_action_tx, audio_action_rx) = mpsc::unbounded_channel();
        let (search_action_tx, search_action_rx) = mpsc::unbounded_channel();
        let (library_action_tx, library_action_rx) = mpsc::unbounded_channel();
        let (view_action_tx, view_action_rx) = mpsc::unbounded_channel();
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
                Action::Popup(popup) => senders.popup.send(popup)?,
                Action::ActiveComponent(action) => {
                    senders.component.send(action)?;
                }
            }
        }

        Ok(())
    }
}
