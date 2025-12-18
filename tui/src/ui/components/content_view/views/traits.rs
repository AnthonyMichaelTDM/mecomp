use super::ViewData;
use mecomp_prost::RecordId;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::Action,
    ui::widgets::tree::{item::CheckTreeItem, state::CheckTreeState},
};

/// Shared functionality for the props of an item view
pub trait ItemViewProps {
    /// Get the id of the thing that this view is displaying
    fn id(&self) -> &RecordId;

    /// Retrieve this view's props from the view data
    fn retrieve(view_data: &ViewData) -> Option<Self>
    where
        Self: Sized;

    /// The title of the view
    fn title() -> &'static str
    where
        Self: Sized;

    /// The string for when no items are checked
    fn none_checked_string() -> &'static str
    where
        Self: Sized;

    fn name() -> &'static str
    where
        Self: Sized;

    #[must_use]
    fn split_area(area: Rect) -> [Rect; 2] {
        let [info_area, content_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)])
            .areas(area);

        [info_area, content_area]
    }

    fn info_widget(&self) -> impl Widget;

    /// Create the tree items for the view
    ///
    /// # Errors
    ///
    /// Returns an error if the tree items cannot be created, e.g. duplicate ids
    fn tree_items(&self) -> Result<Vec<CheckTreeItem<'_, String>>, std::io::Error>;
}

pub trait SortableView {
    fn next_sort(&mut self);

    fn prev_sort(&mut self);

    fn sort_songs(&mut self);

    #[must_use]
    fn footer() -> &'static str
    where
        Self: Sized,
    {
        "s/S: change sort"
    }

    fn handle_extra_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        action_tx: UnboundedSender<Action>,
        tree_state: &mut CheckTreeState<String>,
    );
}

pub trait SortMode<T> {
    #[must_use]
    fn next(&self) -> Self;
    #[must_use]
    fn prev(&self) -> Self;

    fn sort_items(&self, items: &mut [T]);
}
