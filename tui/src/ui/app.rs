//! Handles the main application view logic and state.
//!
//! The `App` struct is responsible for rendering the state of the application to the terminal.
//! The app is updated every tick, and they use the state stores to get the latest state.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::Span,
    widgets::Block,
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::action::{Action, GeneralAction};

use super::{
    colors::{APP_BORDER, APP_BORDER_TEXT, TEXT_NORMAL},
    components::{
        content_view::ContentView, control_panel::ControlPanel, queuebar::QueueBar,
        sidebar::Sidebar, Component, ComponentRender, RenderProps,
    },
    widgets::popups::Popup,
    AppState,
};

#[must_use]
pub struct App {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Props
    props: Props,
    // Components that are always in view
    sidebar: Sidebar,
    queuebar: QueueBar,
    control_panel: ControlPanel,
    content_view: ContentView,
    // (global) Components that are conditionally in view (popups)
    popup: Option<Box<dyn Popup>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Props {
    active_component: ActiveComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

impl App {
    fn get_active_view_component(&self) -> &dyn Component {
        match self.props.active_component {
            ActiveComponent::Sidebar => &self.sidebar,
            ActiveComponent::QueueBar => &self.queuebar,
            ActiveComponent::ControlPanel => &self.control_panel,
            ActiveComponent::ContentView => &self.content_view,
        }
    }

    fn get_active_view_component_mut(&mut self) -> &mut dyn Component {
        match self.props.active_component {
            ActiveComponent::Sidebar => &mut self.sidebar,
            ActiveComponent::QueueBar => &mut self.queuebar,
            ActiveComponent::ControlPanel => &mut self.control_panel,
            ActiveComponent::ContentView => &mut self.content_view,
        }
    }

    /// Move the app with the given state, but only update components that need to be updated.
    ///
    /// in this case, that is the search view
    pub fn move_with_search(self, state: &AppState) -> Self {
        let new = self.content_view.search_view.move_with_state(state);
        Self {
            content_view: ContentView {
                search_view: new,
                ..self.content_view
            },
            ..self
        }
    }

    /// Move the app with the given state, but only update components that need to be updated.
    ///
    /// in this case, that is the queuebar, and the control panel
    pub fn move_with_audio(self, state: &AppState) -> Self {
        Self {
            queuebar: self.queuebar.move_with_state(state),
            control_panel: self.control_panel.move_with_state(state),
            ..self
        }
    }

    /// Move the app with the given state, but only update components that need to be updated.
    ///
    /// in this case, that is the content view
    pub fn move_with_library(self, state: &AppState) -> Self {
        let content_view = self.content_view.move_with_state(state);
        Self {
            content_view,
            ..self
        }
    }

    /// Move the app with the given state, but only update components that need to be updated.
    ///
    /// in this case, that is the content view
    pub fn move_with_view(self, state: &AppState) -> Self {
        let content_view = self.content_view.move_with_state(state);
        Self {
            content_view,
            ..self
        }
    }

    /// Move the app with the given state, but only update components that need to be updated.
    ///
    /// in this case, that is the popup
    pub fn move_with_popup(self, popup: Option<Box<dyn Popup>>) -> Self {
        Self { popup, ..self }
    }
}

impl Component for App {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: Props {
                active_component: state.active_component,
            },
            //
            sidebar: Sidebar::new(state, action_tx.clone()),
            queuebar: QueueBar::new(state, action_tx.clone()),
            control_panel: ControlPanel::new(state, action_tx.clone()),
            content_view: ContentView::new(state, action_tx),
            //
            popup: None,
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        Self {
            sidebar: self.sidebar.move_with_state(state),
            queuebar: self.queuebar.move_with_state(state),
            control_panel: self.control_panel.move_with_state(state),
            content_view: self.content_view.move_with_state(state),
            popup: self.popup.map(|popup| {
                let mut popup = popup;
                popup.update_with_state(state);
                popup
            }),
            ..self
        }
    }

    // defer to the active component
    fn name(&self) -> &str {
        self.get_active_view_component().name()
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // if there is a popup, defer all key handling to it.
        if let Some(popup) = self.popup.as_mut() {
            popup.handle_key_event(key, self.action_tx.clone());
            return;
        }

        // if it's a exit, or navigation command, handle it here.
        // otherwise, defer to the active component
        match key.code {
            KeyCode::Esc => {
                // exit the application
                self.action_tx
                    .send(Action::General(GeneralAction::Exit))
                    .unwrap();
            }
            KeyCode::Tab => {
                self.props.active_component = self.props.active_component.next();
            }
            KeyCode::BackTab => {
                self.props.active_component = self.props.active_component.prev();
            }
            _ => self.get_active_view_component_mut().handle_key_event(key),
        }
    }
}

impl ComponentRender<Rect> for App {
    fn render_border(&self, frame: &mut Frame, area: Rect) -> Rect {
        let block = Block::bordered()
            .title_top(Span::styled(
                "MECOMP",
                Style::default().bold().fg(APP_BORDER_TEXT.into()),
            ))
            .title_bottom(Span::styled(
                "Tab/Shift+Tab to switch focus | Esc to quit",
                Style::default().fg(APP_BORDER_TEXT.into()),
            ))
            .border_style(Style::default().fg(APP_BORDER.into()))
            .style(Style::default().fg(TEXT_NORMAL.into()));
        let app_area = block.inner(area);
        frame.render_widget(block, area);
        app_area
    }

    fn render_content(&self, frame: &mut Frame, area: Rect) {
        let [main_views_area, control_panel_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(4)].as_ref())
            .split(area)
        else {
            panic!("Failed to split frame into areas")
        };

        // figure out the active component, and give it a different colored border
        let (control_panel_focused, sidebar_focused, content_view_focused, queuebar_focused) =
            match self.props.active_component {
                ActiveComponent::ControlPanel => (true, false, false, false),
                ActiveComponent::Sidebar => (false, true, false, false),
                ActiveComponent::ContentView => (false, false, true, false),
                ActiveComponent::QueueBar => (false, false, false, true),
            };

        // render the control panel
        self.control_panel.render(
            frame,
            RenderProps {
                area: control_panel_area,
                is_focused: control_panel_focused,
            },
        );

        // render the main view
        let [sidebar_area, content_view_area, queuebar_area] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Length(19),
                    Constraint::Fill(4),
                    Constraint::Min(25),
                ]
                .as_ref(),
            )
            .split(main_views_area)
        else {
            panic!("Failed to split main views area")
        };

        // render the sidebar
        self.sidebar.render(
            frame,
            RenderProps {
                area: sidebar_area,
                is_focused: sidebar_focused,
            },
        );

        // render the content view
        self.content_view.render(
            frame,
            RenderProps {
                area: content_view_area,
                is_focused: content_view_focused,
            },
        );

        // render the queuebar
        self.queuebar.render(
            frame,
            RenderProps {
                area: queuebar_area,
                is_focused: queuebar_focused,
            },
        );

        // render the popup if there is one
        if let Some(popup) = &self.popup {
            popup.render_popup(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        state::action::{GeneralAction, PopupAction},
        test_utils::setup_test_terminal,
        ui::{
            components::{self, content_view::ActiveView},
            widgets::popups::notification::Notification,
        },
    };
    use mecomp_core::{
        rpc::SearchResult,
        state::{library::LibraryFull, Percent, RepeatMode, StateAudio, StateRuntime},
    };
    use mecomp_storage::db::schemas::song::Song;
    use one_or_many::OneOrMany;
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn song() -> Song {
        Song {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: OneOrMany::One("Test Artist".into()),
            album_artist: OneOrMany::One("Test Album Artist".into()),
            album: "Test Album".into(),
            genre: OneOrMany::One("Test Genre".into()),
            runtime: Duration::from_secs(180),
            track: Some(0),
            disc: Some(0),
            release_year: Some(2021),
            extension: "mp3".into(),
            path: "test.mp3".into(),
        }
    }

    #[rstest]
    #[case::tab(ActiveComponent::Sidebar, ActiveComponent::ContentView)]
    #[case::tab(ActiveComponent::ContentView, ActiveComponent::QueueBar)]
    #[case::tab(ActiveComponent::QueueBar, ActiveComponent::ControlPanel)]
    #[case::tab(ActiveComponent::ControlPanel, ActiveComponent::Sidebar)]
    fn test_handle_key_event_tab(
        #[case] active_component: ActiveComponent,
        #[case] expected: ActiveComponent,
    ) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &AppState {
                active_component,
                ..Default::default()
            },
            tx,
        );

        app.handle_key_event(KeyEvent::from(KeyCode::Tab));

        assert_eq!(app.props.active_component, expected);
    }

    #[rstest]
    #[case::back_tab(ActiveComponent::Sidebar, ActiveComponent::ControlPanel)]
    #[case::back_tab(ActiveComponent::ContentView, ActiveComponent::Sidebar)]
    #[case::back_tab(ActiveComponent::QueueBar, ActiveComponent::ContentView)]
    #[case::back_tab(ActiveComponent::ControlPanel, ActiveComponent::QueueBar)]
    fn test_handle_key_event_back_tab(
        #[case] active_component: ActiveComponent,
        #[case] expected: ActiveComponent,
    ) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &AppState {
                active_component,
                ..Default::default()
            },
            tx,
        );

        app.handle_key_event(KeyEvent::from(KeyCode::BackTab));

        assert_eq!(app.props.active_component, expected);
    }

    #[rstest]
    #[tokio::test]
    #[case::exit(KeyCode::Esc, GeneralAction::Exit)]
    async fn test_handle_key_event_exit(
        #[case] key_code: KeyCode,
        #[case] expected: GeneralAction,
    ) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(&AppState::default(), tx);

        app.handle_key_event(KeyEvent::from(key_code));

        let action = rx.recv().await.unwrap();

        assert_eq!(action, Action::General(expected));
    }

    #[rstest]
    #[case::sidebar(ActiveComponent::Sidebar)]
    #[case::content_view(ActiveComponent::ContentView)]
    #[case::queuebar(ActiveComponent::QueueBar)]
    #[case::control_panel(ActiveComponent::ControlPanel)]
    fn smoke_render(#[case] active_component: ActiveComponent) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = App::new(
            &AppState {
                active_component,
                ..Default::default()
            },
            tx,
        );

        let mut terminal: ratatui::Terminal<ratatui::backend::TestBackend> =
            setup_test_terminal(100, 100);
        let area = terminal.size().unwrap();
        let completed_frame = terminal.draw(|frame| app.render(frame, area));

        assert!(completed_frame.is_ok());
    }

    #[rstest]
    #[case::sidebar(ActiveComponent::Sidebar)]
    #[case::content_view(ActiveComponent::ContentView)]
    #[case::queuebar(ActiveComponent::QueueBar)]
    #[case::control_panel(ActiveComponent::ControlPanel)]
    fn test_render_with_popup(#[case] active_component: ActiveComponent) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = App::new(
            &AppState {
                active_component,
                ..Default::default()
            },
            tx,
        );

        let mut terminal = setup_test_terminal(100, 100);
        let area = terminal.size().unwrap();
        let pre_popup = terminal
            .draw(|frame| app.render(frame, frame.size()))
            .unwrap();

        let app = app.move_with_popup(Some(Box::new(Notification("Hello, World!".into()))));

        let mut terminal = setup_test_terminal(100, 100);
        let post_popup = terminal.draw(|frame| app.render(frame, area)).unwrap();

        assert!(!pre_popup.buffer.diff(post_popup.buffer).is_empty());
    }

    #[rstest]
    #[case::sidebar(ActiveComponent::Sidebar)]
    #[case::content_view(ActiveComponent::ContentView)]
    #[case::queuebar(ActiveComponent::QueueBar)]
    #[case::control_panel(ActiveComponent::ControlPanel)]
    #[tokio::test]
    async fn test_popup_takes_over_key_events(#[case] active_component: ActiveComponent) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &AppState {
                active_component,
                ..Default::default()
            },
            tx,
        );

        let mut terminal = setup_test_terminal(100, 100);
        let area = terminal.size().unwrap();
        let pre_popup = terminal.draw(|frame| app.render(frame, area)).unwrap();

        let popup = Box::new(Notification("Hello, World!".into()));
        app = app.move_with_popup(Some(popup));

        let mut terminal = setup_test_terminal(100, 100);
        let post_popup = terminal.draw(|frame| app.render(frame, area)).unwrap();

        // assert that the popup is rendered
        assert!(!pre_popup.buffer.diff(post_popup.buffer).is_empty());

        // now, send a Esc key event to the app
        app.handle_key_event(KeyEvent::from(KeyCode::Esc));

        // assert that we received a close popup action
        let action = rx.recv().await.unwrap();
        assert_eq!(action, Action::Popup(PopupAction::Close));

        // close the popup (the action handler isn't running so we have to do it manually)
        app = app.move_with_popup(None);

        let mut terminal = setup_test_terminal(100, 100);
        let post_close = terminal.draw(|frame| app.render(frame, area)).unwrap();

        // assert that the popup is no longer rendered
        assert!(!post_popup.buffer.diff(post_close.buffer).is_empty());
        assert!(pre_popup.buffer.diff(post_close.buffer).is_empty());
    }

    #[rstest]
    fn test_move_with_search(song: Song) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let mut app = App::new(&state, tx);

        let state = AppState {
            search: SearchResult {
                songs: vec![song].into_boxed_slice(),
                ..Default::default()
            },
            ..state
        };
        app = app.move_with_search(&state);

        assert_eq!(
            app.content_view.search_view.props.search_results,
            state.search,
        );
    }

    #[rstest]
    fn test_move_with_audio(song: Song) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let mut app = App::new(&state, tx);

        let state = AppState {
            audio: StateAudio {
                queue: vec![song.clone()].into_boxed_slice(),
                queue_position: Some(0),
                current_song: Some(song.clone()),
                repeat_mode: RepeatMode::Once,
                runtime: Some(StateRuntime {
                    seek_position: Duration::from_secs(0),
                    seek_percent: Percent::new(0.0),
                    duration: song.runtime,
                }),
                paused: true,
                muted: false,
                volume: 1.0,
            },
            ..state
        };
        app = app.move_with_audio(&state);

        let components::queuebar::Props {
            queue,
            current_position,
            repeat_mode,
        } = app.queuebar.props;
        assert_eq!(queue, state.audio.queue);
        assert_eq!(current_position, state.audio.queue_position);
        assert_eq!(repeat_mode, state.audio.repeat_mode);

        let components::control_panel::Props {
            is_playing,
            muted,
            volume,
            song_runtime,
            song_title,
            song_artist,
        } = app.control_panel.props;

        assert_eq!(is_playing, !state.audio.paused);
        assert_eq!(muted, state.audio.muted);
        assert_eq!(volume, state.audio.volume);
        assert_eq!(song_runtime, state.audio.runtime);
        assert_eq!(
            song_title,
            state
                .audio
                .current_song
                .as_ref()
                .map(|song| song.title.to_string())
        );
        assert_eq!(
            song_artist,
            state.audio.current_song.as_ref().map(|song| {
                song.artist
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            })
        );
    }

    #[rstest]
    fn test_move_with_library(song: Song) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            active_component: ActiveComponent::ContentView,
            active_view: ActiveView::Songs,
            ..Default::default()
        };
        let mut app = App::new(&state, tx);

        let state = AppState {
            library: LibraryFull {
                songs: vec![song.clone()].into_boxed_slice(),
                ..Default::default()
            },
            ..state
        };
        app = app.move_with_library(&state);

        assert_eq!(app.content_view.songs_view.props.songs, state.library.songs);
    }

    #[rstest]
    fn test_move_with_view(song: Song) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            active_component: ActiveComponent::ContentView,
            active_view: ActiveView::Songs,
            ..Default::default()
        };
        let mut app = App::new(&state, tx);

        let state = AppState {
            active_view: ActiveView::Song(song.id.id.into()),
            ..state
        };
        app = app.move_with_view(&state);

        assert_eq!(app.content_view.props.active_view, state.active_view);
    }

    #[rstest]
    fn test_move_with_popup() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = App::new(&AppState::default(), tx);

        assert!(app.popup.is_none());

        let popup = Box::new(Notification("Hello, World!".into()));
        let app = app.move_with_popup(Some(popup));

        assert!(app.popup.is_some());
    }

    #[rstest]
    #[case::sidebar(ActiveComponent::Sidebar)]
    #[case::content_view(ActiveComponent::ContentView)]
    #[case::queuebar(ActiveComponent::QueueBar)]
    #[case::control_panel(ActiveComponent::ControlPanel)]
    fn test_get_active_view_component(#[case] active_component: ActiveComponent) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            active_component,
            ..Default::default()
        };
        let app = App::new(&state, tx.clone());

        let component = app.get_active_view_component();

        match active_component {
            ActiveComponent::Sidebar => assert_eq!(component.name(), "Sidebar"),
            ActiveComponent::ContentView => assert_eq!(component.name(), "None"), // default content view is the None view, and it defers it's `name()` to the active view
            ActiveComponent::QueueBar => assert_eq!(component.name(), "Queue"),
            ActiveComponent::ControlPanel => assert_eq!(component.name(), "ControlPanel"),
        }

        // assert that the two "get_active_view_component" methods return the same component
        assert_eq!(
            component.name(),
            App::new(&state, tx,).get_active_view_component_mut().name()
        )
    }
}
