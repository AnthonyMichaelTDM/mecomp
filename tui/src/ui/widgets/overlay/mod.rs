pub mod dropdown;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Clear,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, OverlayAction},
    ui::{AppState, components::ComponentRender},
};

pub trait Overlay: for<'a> ComponentRender<Rect> + Send {
    /// The area needed for the overlay to render.
    fn area(&self, terminal_area: Rect) -> Rect;

    fn update_with_state(&mut self, state: &AppState);

    /// Key event handler for the inner overlay content.
    fn inner_handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>);

    /// Shared key handling wrapper.
    fn handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                action_tx.send(Action::Overlay(OverlayAction::Close)).ok();
            }
            _ => self.inner_handle_key_event(key, action_tx),
        }
    }

    /// Mouse event handler for the inner overlay content.
    fn inner_handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: UnboundedSender<Action>,
    );

    /// Shared mouse handling wrapper.
    fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: UnboundedSender<Action>,
    ) {
        if area.contains(Position::new(mouse.column, mouse.row)) {
            self.inner_handle_mouse_event(mouse, area, action_tx);
            return;
        }

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            action_tx.send(Action::Overlay(OverlayAction::Close)).ok();
        }
    }

    fn render_overlay(&mut self, frame: &mut Frame<'_>) {
        let area = self.area(frame.area());
        frame.render_widget(Clear, area);
        self.render(frame, area);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OverlayResult {
    DropdownSelected {
        target_id: u64,
        selected_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayType {
    Dropdown(dropdown::DropdownOverlay),
}

impl OverlayType {
    #[must_use]
    pub fn area(&self, terminal_area: Rect) -> Rect {
        match self {
            Self::Dropdown(overlay) => overlay.area(terminal_area),
        }
    }

    pub fn update_with_state(&mut self, state: &AppState) {
        match self {
            Self::Dropdown(overlay) => overlay.update_with_state(state),
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, action_tx: UnboundedSender<Action>) {
        match self {
            Self::Dropdown(overlay) => overlay.handle_key_event(key, action_tx),
        }
    }

    pub fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: UnboundedSender<Action>,
    ) {
        match self {
            Self::Dropdown(overlay) => overlay.handle_mouse_event(mouse, area, action_tx),
        }
    }

    pub fn render_overlay(&mut self, frame: &mut Frame<'_>) {
        match self {
            Self::Dropdown(overlay) => overlay.render_overlay(frame),
        }
    }
}
