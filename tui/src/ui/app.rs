//! Handles the main application view logic and state.
//!
//! The `App` struct is responsible for rendering the state of the application to the terminal.
//! The app is updated every tick, and they use the state stores to get the latest state.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
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
    AppState,
};

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
    pub const fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::ContentView,
            Self::ContentView => Self::QueueBar,
            Self::QueueBar => Self::ControlPanel,
            Self::ControlPanel => Self::Sidebar,
        }
    }

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

impl ComponentRender<()> for App {
    fn render(&self, frame: &mut Frame, _props: ()) {
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
        let area = block.inner(frame.size());
        frame.render_widget(block, frame.size());

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
                    Constraint::Length(18),
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
    }
}
