use std::collections::HashSet;

use ratatui::layout::{Position, Rect};

use super::{
    flatten::{flatten, Flattened},
    item::CheckTreeItem,
};

/// Keeps the state of what is currently selected and what was opened in a [`CheckTree`](super::CheckTree)
#[derive(Debug, Default, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct CheckTreeState<Identifier> {
    pub(super) offset: usize,
    pub(super) opened: HashSet<Vec<Identifier>>,
    pub(super) selected: Vec<Identifier>,
    pub(super) checked: HashSet<Vec<Identifier>>,
    pub(super) ensure_selected_in_view_on_next_render: bool,

    pub(super) last_area: Rect,
    pub(super) last_biggest_index: usize,
    /// All identifiers open on last render
    pub(super) last_identifiers: Vec<Vec<Identifier>>,
    /// Identifier rendered at `y` on last render
    pub(super) last_rendered_identifiers: Vec<(u16, Vec<Identifier>)>,
}

#[allow(dead_code)]
impl<Identifier> CheckTreeState<Identifier>
where
    Identifier: Clone + PartialEq + Eq + core::hash::Hash,
{
    #[must_use]
    pub const fn get_offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    pub const fn opened(&self) -> &HashSet<Vec<Identifier>> {
        &self.opened
    }

    /// Refers to the current cursor selection.
    #[must_use]
    pub fn selected(&self) -> &[Identifier] {
        &self.selected
    }

    /// Refers to the current checked items.
    #[must_use]
    pub const fn checked(&self) -> &HashSet<Vec<Identifier>> {
        &self.checked
    }

    /// Get a flat list of all currently viewable (including by scrolling) [`CheckTreeItem`]s with this `CheckTreeState`.
    #[must_use]
    pub fn flatten<'text>(
        &self,
        items: &'text [CheckTreeItem<'text, Identifier>],
    ) -> Vec<Flattened<'text, Identifier>> {
        flatten(&self.opened, items, &[])
    }

    /// Selects the given identifier.
    ///
    /// Returns `true` when the selection changed.
    ///
    /// Clear the selection by passing an empty identifier vector:
    pub fn select(&mut self, identifier: Vec<Identifier>) -> bool {
        self.ensure_selected_in_view_on_next_render = true;
        let changed = self.selected != identifier;
        self.selected = identifier;
        changed
    }

    /// Open a tree node.
    /// Returns `true` when it was closed and has been opened.
    /// Returns `false` when it was already open.
    ///
    /// TODO: This should return `false` when it was a leaf node.
    pub fn open(&mut self, identifier: Vec<Identifier>) -> bool {
        if identifier.is_empty() {
            false
        } else {
            self.opened.insert(identifier)
        }
    }

    /// Close a tree node.
    /// Returns `true` when it was open and has been closed.
    /// Returns `false` when it was already closed.
    ///
    /// TODO: This should return `false` when it was a leaf node.
    pub fn close(&mut self, identifier: &[Identifier]) -> bool {
        self.opened.remove(identifier)
    }

    /// Check a tree node
    /// Returns `true` when it was unchecked and has been checked.
    /// Returns `false` when it was already checked.
    ///
    /// TODO: This should return `false` when it was not a leaf node.
    pub fn check(&mut self, identifier: Vec<Identifier>) -> bool {
        if identifier.is_empty() {
            false
        } else {
            // insert the identifier
            self.checked.insert(identifier)
        }
    }

    /// Uncheck a tree node
    /// Returns `true` when it was checked and has been unchecked.
    /// Returns `false` when it was already unchecked.
    ///
    /// TODO: This should return `false` when it was not a leaf node.
    pub fn uncheck(&mut self, identifier: &[Identifier]) -> bool {
        self.checked.remove(identifier)
    }

    /// Toggles a tree node open/close state.
    /// When it is currently open, then [`close`](Self::close) is called. Otherwise [`open`](Self::open).
    ///
    /// Returns `true` when a node is opened / closed.
    /// As toggle always changes something, this only returns `false` when an empty identifier is given.
    ///
    /// TODO: This should return `false` when it was a leaf node.
    pub fn toggle(&mut self, identifier: Vec<Identifier>) -> bool {
        if identifier.is_empty() {
            false
        } else if self.opened.contains(&identifier) {
            self.close(&identifier)
        } else {
            self.open(identifier)
        }
    }

    /// Toggles the currently selected tree node open/close state.
    /// See also [`toggle`](Self::toggle)
    ///
    /// Returns `true` when a node is opened / closed.
    /// As toggle always changes something, this only returns `false` when nothing is selected.
    ///
    /// TODO: This should return `false` when it was a leaf node.
    pub fn toggle_selected(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }

        self.ensure_selected_in_view_on_next_render = true;

        // Reimplement self.close because of multiple different borrows
        let was_open = self.opened.remove(&self.selected);
        if was_open {
            return true;
        }

        self.open(self.selected.clone())
    }

    /// Toggles a tree node checked/unchecked state.
    /// When it is currently checked, then [`uncheck`](Self::uncheck) is called. Otherwise [`check`](Self::check).
    ///
    /// Returns `true` when a node is checked / unchecked.
    /// As toggle always changes something, this only returns `false` when an empty identifier is given.
    ///
    /// TODO: This should return `false` when it was not a leaf node.
    pub fn toggle_check(&mut self, identifier: Vec<Identifier>) -> bool {
        if identifier.is_empty() {
            false
        } else if self.checked.contains(&identifier) {
            self.uncheck(&identifier)
        } else {
            self.check(identifier)
        }
    }

    /// Toggles the currently selected tree node checked/unchecked state.
    /// See also [`toggle_check`](Self::toggle_check)
    /// Returns `true` when a node is checked / unchecked.
    /// As toggle always changes something, this only returns `false` when nothing is selected.
    ///
    /// TODO: This should return `false` when it was not a leaf node.
    pub fn toggle_check_selected(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }

        // Reimplement self.uncheck because of multiple different borrows
        let was_checked = self.checked.remove(&self.selected);
        if was_checked {
            return true;
        }

        self.check(self.selected.clone())
    }

    /// Closes all open nodes, and uncheck all checked nodes.
    ///
    /// Returns `true` when any node was closed or unchecked.
    pub fn reset(&mut self) -> bool {
        if self.opened.is_empty() && self.checked.is_empty() {
            false
        } else {
            self.opened.clear();
            self.checked.clear();
            true
        }
    }

    /// Select the first node.
    ///
    /// Returns `true` when the selection changed.
    pub fn select_first(&mut self) -> bool {
        let identifier = self.last_identifiers.first().cloned().unwrap_or_default();
        self.select(identifier)
    }

    /// Select the last node.
    ///
    /// Returns `true` when the selection changed.
    pub fn select_last(&mut self) -> bool {
        let new_identifier = self.last_identifiers.last().cloned().unwrap_or_default();
        self.select(new_identifier)
    }

    /// Move the current selection with the direction/amount by the given function.
    ///
    /// Returns `true` when the selection changed.
    ///
    /// For more examples take a look into the source code of [`key_up`](Self::key_up) or [`key_down`](Self::key_down).
    /// They are implemented with this method.
    pub fn select_relative<F>(&mut self, change_function: F) -> bool
    where
        F: FnOnce(Option<usize>) -> usize,
    {
        let identifiers = &self.last_identifiers;
        let current_identifier = &self.selected;
        let current_index = identifiers
            .iter()
            .position(|identifier| identifier == current_identifier);
        let new_index = change_function(current_index).min(self.last_biggest_index);
        let new_identifier = identifiers.get(new_index).cloned().unwrap_or_default();
        self.select(new_identifier)
    }

    /// Get the identifier that was rendered for the given position on last render.
    #[must_use]
    pub fn rendered_at(&self, position: Position) -> Option<&[Identifier]> {
        if !self.last_area.contains(position) {
            return None;
        }

        self.last_rendered_identifiers
            .iter()
            .rev()
            .find(|(y, _)| position.y == *y)
            .map(|(_, identifier)| identifier.as_ref())
    }

    /// Ensure the selected [`CheckTreeItem`] is in view on next render
    pub fn scroll_selected_into_view(&mut self) {
        self.ensure_selected_in_view_on_next_render = true;
    }

    /// Scroll the specified amount of lines up
    ///
    /// Returns `true` when the scroll position changed.
    /// Returns `false` when the scrolling has reached the top.
    pub fn scroll_up(&mut self, lines: usize) -> bool {
        let before = self.offset;
        self.offset = self.offset.saturating_sub(lines);
        before != self.offset
    }

    /// Scroll the specified amount of lines down
    ///
    /// Returns `true` when the scroll position changed.
    /// Returns `false` when the scrolling has reached the last [`CheckTreeItem`].
    pub fn scroll_down(&mut self, lines: usize) -> bool {
        let before = self.offset;
        self.offset = self
            .offset
            .saturating_add(lines)
            .min(self.last_biggest_index);
        before != self.offset
    }

    /// Handles the up arrow key.
    /// Moves up in the current depth or to its parent.
    ///
    /// Returns `true` when the selection changed.
    pub fn key_up(&mut self) -> bool {
        self.select_relative(|current| {
            // When nothing is selected, fall back to end
            current.map_or(usize::MAX, |current| current.saturating_sub(1))
        })
    }

    /// Handles the down arrow key.
    /// Moves down in the current depth or into a child node.
    ///
    /// Returns `true` when the selection changed.
    pub fn key_down(&mut self) -> bool {
        self.select_relative(|current| {
            // When nothing is selected, fall back to start
            current.map_or(0, |current| current.saturating_add(1))
        })
    }

    /// Handles the left arrow key.
    /// Closes the currently selected or moves to its parent.
    ///
    /// Returns `true` when the selection or the open state changed.
    pub fn key_left(&mut self) -> bool {
        self.ensure_selected_in_view_on_next_render = true;
        // Reimplement self.close because of multiple different borrows
        let mut changed = self.opened.remove(&self.selected);
        if !changed {
            // Select the parent by removing the leaf from selection
            let popped = self.selected.pop();
            changed = popped.is_some();
        }
        changed
    }

    /// Handles the right arrow key.
    /// Opens the currently selected.
    ///
    /// Returns `true` when it was closed and has been opened.
    /// Returns `false` when it was already open or nothing being selected.
    pub fn key_right(&mut self) -> bool {
        if self.selected.is_empty() {
            false
        } else {
            self.ensure_selected_in_view_on_next_render = true;
            self.open(self.selected.clone())
        }
    }

    /// Handles the space key.
    /// Toggles the whether the current item is selected
    ///
    /// Returns `true` when the selection changed.
    pub fn key_space(&mut self) -> bool {
        self.toggle_check_selected()
    }

    /// Handles a mouse click.
    /// Selects the item at the given position.
    /// If the item is a leaf, it checks it.
    /// It the item is a branch, it toggles it open/closed.
    ///
    /// Returns `true` when the selection or the open state changed.
    /// Returns `false` when nothing was selected.
    pub fn mouse_click(&mut self, position: Position) -> bool {
        let Some(identifier) = self.rendered_at(position) else {
            return false;
        };
        self.select(identifier.to_vec());

        self.ensure_selected_in_view_on_next_render = true;

        self.toggle_check_selected() && self.toggle_selected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_select() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();

        let id: &[&str] = &[];
        assert_eq!(state.select(id.to_vec()), false);
        assert_eq!(state.selected(), id);

        let id = &["a"];
        assert_eq!(state.select(id.clone().to_vec()), true);
        assert_eq!(state.selected(), id);

        let id = &["a", "b"];
        assert_eq!(state.select(id.clone().to_vec()), true);
        assert_eq!(state.selected(), id);

        let id: &[&str] = &[];
        assert_eq!(state.select(id.to_vec()), true);
        assert_eq!(state.selected(), id);

        let id: &[&str] = &[];
        assert_eq!(state.select(id.to_vec()), false);
        assert_eq!(state.selected(), id);
    }

    #[test]
    fn test_open() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        // at first, it's empty
        assert_eq!(state.opened(), &expected);

        // we get false if we "open" nothing
        assert_eq!(state.open(vec![]), false);

        // we get true if we open something that isn't already open
        let id = vec!["a"];
        assert_eq!(state.open(id.clone()), true);
        expected.insert(id.clone());
        assert_eq!(state.opened(), &expected);

        // we get false if we open it again
        let id = vec!["a"];
        assert_eq!(state.open(id.clone()), false);
        assert_eq!(state.opened(), &expected);
    }

    #[test]
    fn test_close() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();

        assert_eq!(state.close(&["a"]), false);

        state.open(vec!["a"]);

        assert_eq!(state.close(&["a"]), true);
        assert_eq!(state.close(&["a"]), false);
    }

    #[test]
    fn test_reset() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();

        assert_eq!(state.reset(), false);

        state.open(vec!["a"]);

        assert_eq!(state.reset(), true);
        assert_eq!(state.reset(), false);

        state.open(vec!["a"]);
        state.open(vec!["a", "b"]);

        assert_eq!(state.reset(), true);
        assert_eq!(state.reset(), false);
    }

    #[test]
    fn test_check() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        // at first, it's empty
        assert_eq!(state.checked(), &expected);

        // we get false if we "check" nothing
        assert_eq!(state.check(vec![]), false);

        // we get true if we check something that isn't already open
        let id = vec!["a"];
        assert_eq!(state.check(id.clone()), true);
        expected.insert(id.clone());
        assert_eq!(state.checked(), &expected);

        // we get false if we check it again
        let id = vec!["a"];
        assert_eq!(state.check(id.clone()), false);
        assert_eq!(state.checked(), &expected);
    }

    #[test]
    fn test_uncheck() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();

        assert_eq!(state.uncheck(&["a"]), false);

        state.check(vec!["a"]);

        assert_eq!(state.uncheck(&["a"]), true);
        assert_eq!(state.uncheck(&["a"]), false);
    }

    #[test]
    fn test_toggle() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        assert_eq!(state.toggle(vec![]), false);

        let id = vec!["a"];
        assert_eq!(state.toggle(id.clone()), true);
        expected.insert(id.clone());
        assert_eq!(state.opened(), &expected);

        assert_eq!(state.toggle(id.clone()), true);
        expected.remove(&id);
        assert_eq!(state.opened(), &expected);
    }

    #[test]
    fn test_toggle_selected() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        assert_eq!(state.toggle_selected(), false);

        let id = vec!["a"];
        state.select(id.clone());

        assert_eq!(state.toggle_selected(), true);
        expected.insert(id.clone());
        assert_eq!(state.opened(), &expected);

        assert_eq!(state.toggle_selected(), true);
        expected.remove(&id);
        assert_eq!(state.opened(), &expected);
    }

    #[test]
    fn test_toggle_check() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        assert_eq!(state.toggle_check(vec![]), false);

        let id = vec!["a"];
        assert_eq!(state.toggle_check(id.clone()), true);
        expected.insert(id.clone());
        assert_eq!(state.checked(), &expected);

        assert_eq!(state.toggle_check(id.clone()), true);
        expected.remove(&id);
        assert_eq!(state.checked(), &expected);
    }

    #[test]
    fn test_toggle_selected_check() {
        let mut state: CheckTreeState<&str> = CheckTreeState::default();
        let mut expected: HashSet<Vec<&str>> = HashSet::default();

        assert_eq!(state.toggle_check_selected(), false);

        let id = vec!["a"];
        state.select(id.clone());

        assert_eq!(state.toggle_check_selected(), true);
        expected.insert(id.clone());
        assert_eq!(state.checked(), &expected);

        assert_eq!(state.toggle_check_selected(), true);
        expected.remove(&id);
        assert_eq!(state.checked(), &expected);
    }
}
