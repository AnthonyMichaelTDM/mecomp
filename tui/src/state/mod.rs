use std::sync::Arc;

use action::Action;
use mecomp_core::{
    rpc::{MusicPlayerClient, SearchResult},
    state::{library::LibraryFull, StateAudio},
};
use ratatui::layout::Rect;
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};

use crate::{
    termination::{Interrupted, Terminator},
    ui::{components::content_view::ActiveView, widgets::popups::Popup},
};

pub mod action;
pub mod audio;
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
}

/// a struct that centralized the receivers for all the state stores.
pub struct Receivers {
    pub audio: UnboundedReceiver<StateAudio>,
    pub search: UnboundedReceiver<SearchResult>,
    pub library: UnboundedReceiver<LibraryFull>,
    pub view: UnboundedReceiver<ActiveView>,
    pub popup: UnboundedReceiver<Option<(Box<dyn Popup>, Rect)>>,
}

impl Dispatcher {
    pub fn new() -> (Self, Receivers) {
        let (audio, audio_rx) = audio::AudioState::new();
        let (search, search_rx) = search::SearchState::new();
        let (library, library_rx) = library::LibraryState::new();
        let (view, view_rx) = view::ViewState::new();
        let (popup, popup_rx) = popup::PopupState::new();

        let dispatcher = Self {
            audio,
            search,
            library,
            view,
            popup,
        };
        let state_receivers = Receivers {
            audio: audio_rx,
            search: search_rx,
            library: library_rx,
            view: view_rx,
            popup: popup_rx,
        };

        (dispatcher, state_receivers)
    }

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
            // the action dispatcher
            Self::action_dispatcher(
                terminator,
                action_rx,
                audio_action_tx,
                search_action_tx,
                library_action_tx,
                view_action_tx,
                popup_action_tx,
            ),
        )?;

        Ok(interrupt_rx.recv().await?)
    }

    async fn action_dispatcher(
        mut terminator: Terminator,
        mut action_rx: UnboundedReceiver<Action>,
        audio_action_tx: UnboundedSender<action::AudioAction>,
        search_action_tx: UnboundedSender<String>,
        library_action_tx: UnboundedSender<action::LibraryAction>,
        view_action_tx: UnboundedSender<ActiveView>,
        popup_action_tx: UnboundedSender<action::PopupAction>,
    ) -> anyhow::Result<()> {
        while let Some(action) = action_rx.recv().await {
            match action {
                Action::Audio(action) => {
                    audio_action_tx.send(action)?;
                }
                Action::Search(query) => {
                    search_action_tx.send(query)?;
                }
                Action::General(action) => match action {
                    action::GeneralAction::Exit => {
                        let _ = terminator.terminate(Interrupted::UserInt);

                        break;
                    }
                },
                Action::Library(action) => {
                    library_action_tx.send(action)?;
                }
                Action::SetCurrentView(view) => {
                    view_action_tx.send(view)?;
                }
                Action::Popup(popup) => popup_action_tx.send(popup)?,
            }
        }

        Ok(())
    }
}
