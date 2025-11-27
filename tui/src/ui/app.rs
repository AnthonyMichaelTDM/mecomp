//! Handles the main application view logic and state.
//!
//! The `App` struct is responsible for rendering the state of the application to the terminal.
//! The app is updated every tick, and they use the state stores to get the latest state.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Style, Stylize},
    text::Span,
    widgets::Block,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::{
    action::{Action, ComponentAction, GeneralAction, LibraryAction},
    component::ActiveComponent,
};

use super::{
    AppState,
    colors::{APP_BORDER, APP_BORDER_TEXT, TEXT_NORMAL},
    components::{
        Component, ComponentRender, RenderProps, content_view::ContentView,
        control_panel::ControlPanel, queuebar::QueueBar, sidebar::Sidebar,
    },
    widgets::popups::Popup,
};

#[must_use]
pub struct App {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// active component
    active_component: ActiveComponent,
    // Components that are always in view
    sidebar: Sidebar,
    queuebar: QueueBar,
    control_panel: ControlPanel,
    content_view: ContentView,
    // (global) Components that are conditionally in view (popups)
    popup: Option<Box<dyn Popup>>,
}

impl App {
    fn get_active_view_component(&self) -> &dyn Component {
        match self.active_component {
            ActiveComponent::Sidebar => &self.sidebar,
            ActiveComponent::QueueBar => &self.queuebar,
            ActiveComponent::ControlPanel => &self.control_panel,
            ActiveComponent::ContentView => &self.content_view,
        }
    }

    fn get_active_view_component_mut(&mut self) -> &mut dyn Component {
        match self.active_component {
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
    /// in this case, that is the active component
    pub fn move_with_component(self, state: &AppState) -> Self {
        Self {
            active_component: state.active_component,
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
            active_component: state.active_component,
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
            // exit the application
            KeyCode::Esc => {
                self.action_tx
                    .send(Action::General(GeneralAction::Exit))
                    .unwrap();
            }
            // cycle through the components
            KeyCode::Tab => self
                .action_tx
                .send(Action::ActiveComponent(ComponentAction::Next))
                .unwrap(),
            KeyCode::BackTab => self
                .action_tx
                .send(Action::ActiveComponent(ComponentAction::Previous))
                .unwrap(),
            // Refresh the active component
            KeyCode::F(5) => self
                .action_tx
                .send(Action::Library(LibraryAction::Update))
                .unwrap(),
            // defer to the active component
            _ => self.get_active_view_component_mut().handle_key_event(key),
        }
    }

    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) {
        // if there is a popup, defer all mouse handling to it.
        if let Some(popup) = self.popup.as_mut() {
            popup.handle_mouse_event(mouse, popup.area(area), self.action_tx.clone());
            return;
        }

        // adjust area to exclude the border
        let area = area.inner(Margin::new(1, 1));

        // defer to the component that the mouse is in
        let mouse_position = Position::new(mouse.column, mouse.row);
        let Areas {
            control_panel,
            sidebar,
            content_view,
            queuebar,
        } = split_area(area);

        if control_panel.contains(mouse_position) {
            self.control_panel.handle_mouse_event(mouse, control_panel);
        } else if sidebar.contains(mouse_position) {
            self.sidebar.handle_mouse_event(mouse, sidebar);
        } else if content_view.contains(mouse_position) {
            self.content_view.handle_mouse_event(mouse, content_view);
        } else if queuebar.contains(mouse_position) {
            self.queuebar.handle_mouse_event(mouse, queuebar);
        }
    }
}

#[derive(Debug)]
struct Areas {
    pub control_panel: Rect,
    pub sidebar: Rect,
    pub content_view: Rect,
    pub queuebar: Rect,
}

fn split_area(area: Rect) -> Areas {
    let [main_views, control_panel] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(4)].as_ref())
        .split(area)
    else {
        panic!("Failed to split frame into areas")
    };

    let [sidebar, content_view, queuebar] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(19),
                Constraint::Fill(4),
                Constraint::Min(25),
            ]
            .as_ref(),
        )
        .split(main_views)
    else {
        panic!("Failed to split main views area")
    };

    Areas {
        control_panel,
        sidebar,
        content_view,
        queuebar,
    }
}

impl ComponentRender<Rect> for App {
    fn render_border(&self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        let block = Block::bordered()
            .title_top(Span::styled(
                "MECOMP",
                Style::default().bold().fg((*APP_BORDER_TEXT).into()),
            ))
            .title_bottom(Span::styled(
                "Tab/Shift+Tab to switch focus | Esc to quit | F5 to refresh",
                Style::default().fg((*APP_BORDER_TEXT).into()),
            ))
            .border_style(Style::default().fg((*APP_BORDER).into()))
            .style(Style::default().fg((*TEXT_NORMAL).into()));
        let app_area = block.inner(area);
        debug_assert_eq!(area.inner(Margin::new(1, 1)), app_area);

        frame.render_widget(block, area);
        app_area
    }

    fn render_content(&self, frame: &mut Frame<'_>, area: Rect) {
        let Areas {
            control_panel,
            sidebar,
            content_view,
            queuebar,
        } = split_area(area);

        // figure out the active component, and give it a different colored border
        let (control_panel_focused, sidebar_focused, content_view_focused, queuebar_focused) =
            match self.active_component {
                ActiveComponent::ControlPanel => (true, false, false, false),
                ActiveComponent::Sidebar => (false, true, false, false),
                ActiveComponent::ContentView => (false, false, true, false),
                ActiveComponent::QueueBar => (false, false, false, true),
            };

        // render the control panel
        self.control_panel.render(
            frame,
            RenderProps {
                area: control_panel,
                is_focused: control_panel_focused,
            },
        );

        // render the sidebar
        self.sidebar.render(
            frame,
            RenderProps {
                area: sidebar,
                is_focused: sidebar_focused,
            },
        );

        // render the content view
        self.content_view.render(
            frame,
            RenderProps {
                area: content_view,
                is_focused: content_view_focused,
            },
        );

        // render the queuebar
        self.queuebar.render(
            frame,
            RenderProps {
                area: queuebar,
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
        state::action::PopupAction,
        test_utils::setup_test_terminal,
        ui::{
            components::{self, content_view::ActiveView},
            widgets::popups::notification::Notification,
        },
    };
    use crossterm::event::KeyModifiers;
    use mecomp_core::state::{Percent, RepeatMode, StateAudio, StateRuntime, Status};
    use mecomp_prost::{LibraryBrief, SearchResult};
    use mecomp_storage::db::schemas::song::{Song, SongBrief};
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};
    use tokio::sync::mpsc::unbounded_channel;

    #[fixture]
    fn song() -> SongBrief {
        SongBrief {
            id: Song::generate_id(),
            title: "Test Song".into(),
            artist: "Test Artist".to_string().into(),
            album_artist: "Test Album Artist".to_string().into(),
            album: "Test Album".into(),
            genre: "Test Genre".to_string().into(),
            runtime: Duration::from_secs(180),
            track: Some(0),
            disc: Some(0),
            release_year: Some(2021),
            path: "test.mp3".into(),
        }
    }

    #[rstest]
    #[case::tab(KeyCode::Tab, Action::ActiveComponent(ComponentAction::Next))]
    #[case::back_tab(KeyCode::BackTab, Action::ActiveComponent(ComponentAction::Previous))]
    #[case::esc(KeyCode::Esc, Action::General(GeneralAction::Exit))]
    fn test_actions(#[case] key_code: KeyCode, #[case] expected: Action) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(&AppState::default(), tx);

        app.handle_key_event(KeyEvent::from(key_code));

        let action = rx.blocking_recv().unwrap();

        assert_eq!(action, expected);
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

        let (mut terminal, area) = setup_test_terminal(100, 100);
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

        let (mut terminal, area) = setup_test_terminal(100, 100);
        let pre_popup = terminal.draw(|frame| app.render(frame, area)).unwrap();

        let app = app.move_with_popup(Some(Box::new(Notification::new(
            "Hello, World!".into(),
            unbounded_channel().0,
        ))));

        let (mut terminal, area) = setup_test_terminal(100, 100);
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

        let (mut terminal, area) = setup_test_terminal(100, 100);
        let pre_popup = terminal.draw(|frame| app.render(frame, area)).unwrap();

        let popup = Box::new(Notification::new(
            "Hello, World!".into(),
            unbounded_channel().0,
        ));
        app = app.move_with_popup(Some(popup));

        let (mut terminal, area) = setup_test_terminal(100, 100);
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

        let (mut terminal, area) = setup_test_terminal(100, 100);
        let post_close = terminal.draw(|frame| app.render(frame, area)).unwrap();

        // assert that the popup is no longer rendered
        assert!(!post_popup.buffer.diff(post_close.buffer).is_empty());
        assert!(pre_popup.buffer.diff(post_close.buffer).is_empty());
    }

    #[rstest]
    fn test_move_with_search(song: SongBrief) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let mut app = App::new(&state, tx);

        let state = AppState {
            search: SearchResult {
                songs: vec![song.into()],
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
    fn test_move_with_audio(song: SongBrief) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let mut app = App::new(&state, tx);

        let state = AppState {
            audio: StateAudio {
                queue: vec![song.clone()].into_boxed_slice(),
                queue_position: Some(0),
                current_song: Some(song.clone()),
                repeat_mode: RepeatMode::One,
                runtime: Some(StateRuntime {
                    seek_position: Duration::from_secs(0),
                    seek_percent: Percent::new(0.0),
                    duration: song.runtime,
                }),
                status: Status::Stopped,
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

        assert_eq!(is_playing, !state.audio.paused());
        assert_eq!(muted, state.audio.muted);
        assert!(
            f32::EPSILON > (volume - state.audio.volume).abs(),
            "{} != {}",
            volume,
            state.audio.volume
        );
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
            state
                .audio
                .current_song
                .as_ref()
                .map(|song| { song.artist.as_slice().join(", ") })
        );
    }

    #[rstest]
    fn test_move_with_library(song: SongBrief) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            active_component: ActiveComponent::ContentView,
            active_view: ActiveView::Songs,
            ..Default::default()
        };
        let mut app = App::new(&state, tx);

        let state = AppState {
            library: LibraryBrief {
                songs: vec![song.into()],
                ..Default::default()
            },
            ..state
        };
        app = app.move_with_library(&state);

        assert_eq!(app.content_view.songs_view.props.songs, state.library.songs);
    }

    #[rstest]
    fn test_move_with_view(song: SongBrief) {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            active_component: ActiveComponent::ContentView,
            active_view: ActiveView::Songs,
            ..Default::default()
        };
        let mut app = App::new(&state, tx);

        let state = AppState {
            active_view: ActiveView::Song(song.id.key().to_string().into()),
            ..state
        };
        app = app.move_with_view(&state);

        assert_eq!(app.content_view.props.active_view, state.active_view);
    }

    #[test]
    fn test_move_with_component() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = App::new(&AppState::default(), tx);

        assert_eq!(app.active_component, ActiveComponent::Sidebar);

        let state = AppState {
            active_component: ActiveComponent::QueueBar,
            ..Default::default()
        };
        let app = app.move_with_component(&state);

        assert_eq!(app.active_component, ActiveComponent::QueueBar);
    }

    #[rstest]
    fn test_move_with_popup() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = App::new(&AppState::default(), tx);

        assert!(app.popup.is_none());

        let popup = Box::new(Notification::new(
            "Hello, World!".into(),
            unbounded_channel().0,
        ));
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
        );
    }

    #[test]
    fn test_click_to_focus() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(&AppState::default(), tx);

        let (mut terminal, area) = setup_test_terminal(100, 100);
        let _frame = terminal.draw(|frame| app.render(frame, area)).unwrap();

        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_mouse_event(mouse, area);

        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveComponent(ComponentAction::Set(ActiveComponent::Sidebar))
        );

        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 50,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_mouse_event(mouse, area);

        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveComponent(ComponentAction::Set(ActiveComponent::ContentView))
        );

        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 90,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_mouse_event(mouse, area);

        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveComponent(ComponentAction::Set(ActiveComponent::QueueBar))
        );

        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 60,
            row: 98,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_mouse_event(mouse, area);

        let action = rx.blocking_recv().unwrap();
        assert_eq!(
            action,
            Action::ActiveComponent(ComponentAction::Set(ActiveComponent::ControlPanel))
        );
    }
}
