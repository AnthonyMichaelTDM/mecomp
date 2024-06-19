//! This module contains the implementations of the TUI.
//!
//! The app is responsible for rendering the state of the application to the terminal.
//!
//! The app is updated every tick, and they use the state stores to get the latest state.

pub mod app;
pub mod components;
pub mod widgets;

use std::{
    io::{self, Stdout},
    time::Duration,
};

use anyhow::Context;
use app::{ActiveComponent, App};
use components::{content_view::ActiveView, Component, ComponentRender};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mecomp_core::{
    rpc::SearchResult,
    state::{library::LibraryFull, StateAudio},
};
use ratatui::prelude::*;
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver},
};
use tokio_stream::StreamExt;

use crate::{
    state::{action::Action, Receivers},
    termination::Interrupted,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub active_component: ActiveComponent,
    pub audio: StateAudio,
    pub search: SearchResult,
    pub library: LibraryFull,
    pub view: ActiveView,
}

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

#[allow(clippy::module_name_repetitions)]
pub struct UiManager {
    action_tx: mpsc::UnboundedSender<Action>,
}

impl UiManager {
    pub fn new() -> (Self, UnboundedReceiver<Action>) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        (Self { action_tx }, action_rx)
    }

    pub async fn main_loop(
        self,
        mut state_rx: Receivers,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // consume the first state to initialize the ui app
        let mut state = AppState {
            active_component: ActiveComponent::default(),
            audio: state_rx.audio.recv().await.unwrap_or_default(),
            search: state_rx.search.recv().await.unwrap_or_default(),
            library: state_rx.library.recv().await.unwrap_or_default(),
            view: state_rx.view.recv().await.unwrap_or_default(),
        };
        let mut app = App::new(&state, self.action_tx.clone());

        let mut terminal = setup_terminal()?;
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut crossterm_events = EventStream::new();

        let result: anyhow::Result<Interrupted> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                // Catch and handle crossterm events
               maybe_event = crossterm_events.next() => match maybe_event {
                    Some(Ok(Event::Key(key)))  => {
                        app.handle_key_event(key);
                    },
                    None => break Ok(Interrupted::UserInt),
                    _ => (),
                },
                // Handle state updates
                Some(audio) = state_rx.audio.recv() => {
                    state = AppState {
                        audio,
                        ..state
                    };
                    app = app.move_with_audio(&state);
                },
                Some(search) = state_rx.search.recv() => {
                    state = AppState {
                        search,
                        ..state
                    };
                    app = app.move_with_search(&state);
                },
                Some(library) = state_rx.library.recv() => {
                    state = AppState {
                        library,
                        ..state
                    };
                    app = app.move_with_library(&state);
                },
                Some(view) = state_rx.view.recv() => {
                    state = AppState {
                        view,
                        ..state
                    };
                    app = app.move_with_view(&state);
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break Ok(interrupted);
                }
            }

            if let Err(err) = terminal
                .draw(|frame| app.render(frame, ()))
                .context("could not render to the terminal")
            {
                break Err(err);
            }
        };

        restore_terminal(&mut terminal)?;

        result
    }
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(terminal.show_cursor()?)
}

pub fn init_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

        original_hook(panic_info);
    }));
}
