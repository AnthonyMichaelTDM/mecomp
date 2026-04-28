//! This is the implementation of the query builder for dynamic playlists
//!
//! pending TODOs:
//! - [ ] fix issue where dropdown overlay isn't updating its corresponding dropbox
//! - [ ] implement mouse event handling
//! - [ ] implement text overlay for VALUEs (text, integer, set)
//! - [ ] instead of this messy FlatNode and leaf_at_mut/group_at_mut mess where we have a tree that we flatten, parse, navigate, etc. multiple times per frame.. design the editor s.t. it maintains the underlying "raw text" and parses that to generate a list of dyn objects implementing some new trait that defines the behavior of "row items" (group, leaf, "add clause", "add group").

pub mod events;
pub mod render;
pub mod state;
mod utils;

use crossterm::event::{KeyEvent, MouseEvent};
use mecomp_storage::db::schemas::dynamic::query::Query;
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::action::Action;
use crate::ui::widgets::overlay::OverlayResult;

pub use self::state::QueryBuilderState;

/// High-level query builder widget.
///
/// Owns a [`QueryBuilderState`] and exposes clean methods for embedding into other components.
#[derive(Debug, Default)]
pub struct QueryBuilder {
    pub state: QueryBuilderState,
}

impl QueryBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the compiled `Query` if the current state represents a valid query.
    #[must_use]
    pub fn query(&self) -> Option<Query> {
        self.state.try_to_query()
    }

    /// Load an existing `Query` into the builder.
    pub fn set_query(&mut self, q: &Query) {
        self.state.load_query(q);
    }

    /// Reset the builder to an empty initial state.
    pub fn clear(&mut self) {
        self.state = QueryBuilderState::default();
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, action_tx: &UnboundedSender<Action>) {
        if let Some(action) = events::handle_key_event(&mut self.state, key) {
            action_tx.send(action).ok();
        }
    }

    pub fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        action_tx: &UnboundedSender<Action>,
    ) {
        if let Some(action) = events::handle_mouse_event(&mut self.state, mouse, area) {
            action_tx.send(action).ok();
        }
    }

    pub fn apply_overlay_result(&mut self, result: &OverlayResult) -> bool {
        self.state.apply_overlay_result(result)
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, is_focused: bool) {
        render::render_query_builder(frame, &mut self.state, area, is_focused);
    }
}
