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
    ui::components::content_view::ActiveView,
};

pub mod action;
pub mod audio;
pub mod library;
pub mod search;
pub mod view;

/// an all-in-one dispactcher for managing state updates.
pub struct Dispatcher {
    audio: audio::AudioState,
    search: search::SearchState,
    library: library::LibraryState,
    view: view::ViewStore,
}

/// a struct that centralized the receivers for all the state stores.
pub struct StateReceivers {
    pub audio_rx: UnboundedReceiver<StateAudio>,
    pub search_rx: UnboundedReceiver<SearchResult>,
    pub library_rx: UnboundedReceiver<LibraryFull>,
    pub view_rx: UnboundedReceiver<ActiveView>,
}

impl Dispatcher {
    pub fn new() -> (Self, StateReceivers) {
        let (audio, audio_rx) = audio::AudioState::new();
        let (search, search_rx) = search::SearchState::new();
        let (library, library_rx) = library::LibraryState::new();
        let (view, view_rx) = view::ViewStore::new();

        let dispatcher = Dispatcher {
            audio,
            search,
            library,
            view,
        };
        let state_receivers = StateReceivers {
            audio_rx,
            search_rx,
            library_rx,
            view_rx,
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
            // the action dispatcher
            Self::action_dispatcher(
                terminator,
                action_rx,
                audio_action_tx,
                search_action_tx,
                library_action_tx,
                view_action_tx,
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
            }
        }

        Ok(())
    }
}
