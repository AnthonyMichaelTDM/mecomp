use super::ViewData;
use mecomp_prost::RecordId;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Scrollbar, Widget},
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

    /// Handle any extra key events specific to this view
    #[inline]
    fn handle_extra_key_events(
        &mut self,
        _: crossterm::event::KeyEvent,
        _: UnboundedSender<Action>,
        _: &mut CheckTreeState<String>,
    ) {
    }

    /// Optionally define an additional footer to give instructions for the extra key events
    #[must_use]
    fn extra_footer() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }

    /// Optionally use a scrollbar when rendering the tree
    #[must_use]
    fn scrollbar() -> Option<Scrollbar<'static>>
    where
        Self: Sized,
    {
        None
    }
}

pub trait SortableViewProps<Item> {
    /// This exists essentially because the generic `SortableItemView` doesn't actually know what its items are, per se
    fn sort_items(&mut self, sort_mode: &impl SortMode<Item>);
}

pub trait SortMode<T>: ToString + Default {
    #[must_use]
    fn next(&self) -> Self;
    #[must_use]
    fn prev(&self) -> Self;

    fn sort_items(&self, items: &mut [T]);
}
