//! UI-side data model for the query builder.

use std::str::FromStr;

use mecomp_storage::db::schemas::dynamic::query::{
    Clause, Compile as _, CompoundClause, CompoundKind, Query,
};
use ratatui::layout::Rect;
use strum::IntoEnumIterator;

use crate::ui::widgets::{
    dropdown::DropdownState,
    input_box::InputBoxState,
    overlay::OverlayResult,
    query_builder::utils::{LeafFocus, UiValue, flatten_tree},
};

use super::utils::{CursorPath, UiClause, UiCompoundKind, UiGroup, UiLeafClause};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuilderMode {
    #[default]
    Visual,
    RawText,
}

#[derive(Debug, Clone, Copy)]
pub enum ClickableAction {
    /// Click on a group's kind dropdown
    GroupKind,
    /// Click on a leaf's field dropdown
    LeafField,
    /// Click on a leaf's operator dropdown
    LeafOperator,
    /// Click on a leaf's value field (open overlay)
    LeafValue,
    /// Click on a delete button for this element
    Delete,
    /// Click on the "add clause" button
    AddClause,
    /// Click on the "add group" button
    AddGroup,
}

impl ClickableAction {
    #[must_use]
    pub const fn region(self, area: Rect, path: Vec<usize>, flat_index: usize) -> ClickableRegion {
        // Determine leaf focus based on action type
        let leaf_focus = match self {
            Self::LeafField => Some(LeafFocus::Field),
            Self::LeafOperator => Some(LeafFocus::Operator),
            Self::LeafValue => Some(LeafFocus::Value),
            _ => None,
        };

        ClickableRegion {
            area,
            action: self,
            path,
            flat_index,
            leaf_focus,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClickableRegion {
    /// The screen area of this clickable region
    pub area: Rect,
    /// The action to perform when clicked
    pub action: ClickableAction,
    /// The path to the element in the tree
    pub path: Vec<usize>,
    /// The flat index in the flattened tree (for cursor positioning)
    pub flat_index: usize,
    /// For leaf nodes: which sub-element is this? (field, operator, value)
    pub leaf_focus: Option<LeafFocus>,
}

#[derive(Debug, Clone)]
/// Full state of the visual query builder.
pub struct QueryBuilderState {
    pub mode: BuilderMode,
    /// The root group (always a group, never a bare leaf at the top level).
    pub root: UiGroup,
    /// Cursor position in the flat list.
    pub cursor: CursorPath,
    /// Raw-text input mode.
    pub raw_input: InputBoxState,
    /// Is the raw input currently a valid query?
    pub raw_input_valid: bool,
    /// Clickable regions for mouse handling (populated during render)
    pub clickable_regions: Vec<ClickableRegion>,
}

impl Default for QueryBuilderState {
    fn default() -> Self {
        Self {
            mode: BuilderMode::Visual,
            root: UiGroup::new(CompoundKind::And),
            cursor: CursorPath::new(3), // default flat length: 1 leaf + 2 buttons
            raw_input: InputBoxState::new(),
            raw_input_valid: false,
            clickable_regions: Vec::new(),
        }
    }
}

impl QueryBuilderState {
    /// Apply the given overlay result to the state
    pub fn apply_overlay_result(&mut self, result: &OverlayResult) -> bool {
        let flat = flatten_tree(&self.root);
        let Some(current_node) = flat.get(self.cursor.flat_index) else {
            return false;
        };

        // find the available dropdowns at the currently selected node
        let success = match result {
            OverlayResult::DropdownSelected { target_id, .. } => {
                if *target_id == 0 {
                    // this is a dropdown for a compound clause
                    let Some(group) = self.group_at_mut(&current_node.path) else {
                        return false;
                    };
                    group.kind_dd.apply_overlay_result(result)
                } else if *target_id > 0 && *target_id <= 2 {
                    // this is a dropdown for a leaf clause
                    let Some(leaf) = self.leaf_at_mut(&current_node.path) else {
                        return false;
                    };
                    match target_id {
                        1 => leaf.field_dd.apply_overlay_result(result),
                        2 => leaf.operator_dd.apply_overlay_result(result),
                        _ => false,
                    }
                } else {
                    false
                }
            }
            OverlayResult::TextInputted { target_id, text } => {
                if *target_id == 3
                    && let Some(leaf) = self.leaf_at_mut(&current_node.path)
                    && let UiValue::Text(input) | UiValue::Integer(input) = &mut leaf.value
                {
                    input.clone_from(text);
                    true
                } else {
                    false
                }
            }
            OverlayResult::SetEdited { target_id, items } => {
                if *target_id == 3
                    && let Some(leaf) = self.leaf_at_mut(&current_node.path)
                    && let UiValue::Set(set_items) = &mut leaf.value
                {
                    set_items.clone_from(items);
                    true
                } else {
                    false
                }
            }
        };

        // Sync raw text with visual tree after any successful modification
        if success {
            self.sync_raw_from_visual();
        }
        success
    }

    /// Try to compile the current state to a `Query`.
    #[must_use]
    pub fn try_to_query(&self) -> Option<Query> {
        match self.mode {
            BuilderMode::RawText => Query::from_str(self.raw_input.text()).ok(),
            BuilderMode::Visual => self.root.try_to_clause().map(|root| Query { root }),
        }
    }

    /// Load a `Query` into the visual builder.
    pub fn load_query(&mut self, query: &Query) {
        self.root = clause_to_ui_group(&query.root);
        let flat_len = flatten_tree(&self.root).len();
        self.cursor = CursorPath::new(flat_len);
        // also update raw input for when user toggles mode
        self.raw_input.set_text(&query.compile_for_storage());
        self.raw_input_valid = true;
        self.mode = BuilderMode::Visual;
    }

    /// Toggle between visual and raw-text mode, syncing state in both directions.
    pub fn toggle_mode(&mut self) {
        match self.mode {
            BuilderMode::Visual => {
                // capture current visual query as text
                self.sync_raw_from_visual();
                self.mode = BuilderMode::RawText;
            }
            BuilderMode::RawText => {
                // parse raw text back to visual
                if let Ok(q) = Query::from_str(self.raw_input.text()) {
                    self.root = clause_to_ui_group(&q.root);
                    self.raw_input_valid = true;
                }
                self.mode = BuilderMode::Visual;
            }
        }
    }

    /// Update raw input validity.
    pub fn update_raw_validity(&mut self) {
        self.raw_input_valid = Query::from_str(self.raw_input.text()).is_ok();
    }

    /// Synchronize raw text representation from the visual tree.
    /// Called after any modification to ensure both representations stay in sync.
    fn sync_raw_from_visual(&mut self) {
        if let Some(q) = self.root.try_to_clause().map(|root| Query { root }) {
            self.raw_input.set_text(&q.compile_for_storage());
            self.raw_input_valid = true;
        } else {
            self.raw_input_valid = false;
        }
    }

    // ── Mutators ─────────────────────────────────────────────────────────────

    /// Add a new leaf clause to the group at `path` (empty path = root).
    pub fn add_leaf_at(&mut self, path: &[usize]) {
        if let Some(group) = self.group_at_mut(path) {
            group.clauses.push(UiClause::Leaf(UiLeafClause::new()));
        }
        self.sync_raw_from_visual();
    }

    /// Add a new sub-group to the group at `path`.
    pub fn add_group_at(&mut self, path: &[usize]) {
        if let Some(group) = self.group_at_mut(path) {
            let new_group = UiGroup::new(CompoundKind::And);
            group.clauses.push(UiClause::Group(new_group));
        }
        self.sync_raw_from_visual();
    }

    /// Remove the child at position `child_index` in the group at `path`.
    /// Guards: won't remove if the parent group would drop below 1 child.
    pub fn remove_child_at(&mut self, path: &[usize]) {
        guard_remove(&mut self.root, path);
        self.sync_raw_from_visual();
    }

    /// Return a mutable reference to the group node at the given path.
    pub fn group_at_mut<'a>(&'a mut self, path: &[usize]) -> Option<&'a mut UiGroup> {
        navigate_to_group_mut(&mut self.root, path)
    }

    /// Return a mutable reference to the leaf at the given path.
    pub fn leaf_at_mut<'a>(&'a mut self, path: &[usize]) -> Option<&'a mut UiLeafClause> {
        navigate_to_leaf_mut(&mut self.root, path)
    }
}

// ── Navigation helpers ───────────────────────────────────────────────────────

fn navigate_to_group_mut<'a>(root: &'a mut UiGroup, path: &[usize]) -> Option<&'a mut UiGroup> {
    if path.is_empty() {
        return Some(root);
    }
    let idx = path[0];
    let child = root.clauses.get_mut(idx)?;
    match child {
        UiClause::Group(g) => navigate_to_group_mut(g, &path[1..]),
        UiClause::Leaf(_) => None,
    }
}

fn navigate_to_leaf_mut<'a>(root: &'a mut UiGroup, path: &[usize]) -> Option<&'a mut UiLeafClause> {
    if path.is_empty() {
        return None; // root is always a group
    }
    if path.len() == 1 {
        let idx = path[0];
        let child = root.clauses.get_mut(idx)?;
        return match child {
            UiClause::Leaf(l) => Some(l),
            UiClause::Group(_) => None,
        };
    }
    // recurse into group
    let idx = path[0];
    let child = root.clauses.get_mut(idx)?;
    match child {
        UiClause::Group(g) => navigate_to_leaf_mut(g, &path[1..]),
        UiClause::Leaf(_) => None,
    }
}

fn guard_remove(root: &mut UiGroup, path: &[usize]) {
    if path.is_empty() {
        return; // can't remove root
    }
    // path[-1] is the index of the child to remove; path[..-1] is the parent group path
    let (parent_path, child_idx_slice) = path.split_at(path.len() - 1);
    let child_idx = child_idx_slice[0];

    if let Some(parent) = navigate_to_group_mut(root, parent_path)
        && parent.clauses.len() > 1
    {
        parent.clauses.remove(child_idx);
    }
}

// ── Conversion from storage types ───────────────────────────────────────────

/// Convert a storage `Clause` to a `UiGroup`.
///
/// - A `Compound` becomes a proper `UiGroup` with N-arified children.
/// - A bare `Leaf` becomes a `UiGroup` wrapping a single leaf.
fn clause_to_ui_group(clause: &Clause) -> UiGroup {
    match clause {
        Clause::Compound(c) => compound_to_ui_group(c),
        Clause::Leaf(l) => {
            let mut ui_leaf = UiLeafClause::new();
            ui_leaf.load_leaf(l);
            UiGroup {
                kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                clauses: vec![UiClause::Leaf(ui_leaf)],
            }
        }
    }
}

fn compound_to_ui_group(compound: &CompoundClause) -> UiGroup {
    let kind = compound.kind;
    let mut kind_dd = DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind));
    let _ = kind_dd.select_by_text(UiCompoundKind(kind).to_string().as_str());

    // Flatten N-ary: if nested compounds have the same kind, absorb their children.
    let mut children: Vec<UiClause> = Vec::new();
    flatten_compound_children(&compound.clauses, kind, &mut children);

    UiGroup {
        kind_dd,
        clauses: children,
    }
}

fn flatten_compound_children(clauses: &[Clause], kind: CompoundKind, out: &mut Vec<UiClause>) {
    for clause in clauses {
        match clause {
            Clause::Compound(c) if c.kind == kind => {
                // absorb children of same-kind compound
                flatten_compound_children(&c.clauses, kind, out);
            }
            Clause::Compound(c) => {
                out.push(UiClause::Group(compound_to_ui_group(c)));
            }
            Clause::Leaf(l) => {
                let mut ui_leaf = UiLeafClause::new();
                ui_leaf.load_leaf(l);
                out.push(UiClause::Leaf(ui_leaf));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mecomp_storage::db::schemas::dynamic::query::{
        Compile, Field, LeafClause, Operator, Value,
    };
    use pretty_assertions::assert_eq;

    fn leaf(field: Field, op: Operator, val: &str) -> Clause {
        Clause::Leaf(LeafClause {
            left: Value::Field(field),
            operator: op,
            right: Value::String(val.to_string()),
        })
    }

    #[test]
    fn test_round_trip_simple_leaf() {
        let clause = leaf(Field::Title, Operator::Equal, "foo");
        let query = Query { root: clause };

        let mut state = QueryBuilderState::default();
        state.load_query(&query);

        let result = state.try_to_query().unwrap();
        assert_eq!(result.compile_for_storage(), "title = \"foo\"");
    }

    #[test]
    fn test_round_trip_compound() {
        let clause = Clause::Compound(CompoundClause {
            kind: CompoundKind::And,
            clauses: vec![
                leaf(Field::Title, Operator::Equal, "foo"),
                leaf(Field::Album, Operator::Like, "bar"),
            ],
        });
        let query = Query { root: clause };

        let mut state = QueryBuilderState::default();
        state.load_query(&query);

        let result = state.try_to_query().unwrap();
        assert_eq!(
            result.compile_for_storage(),
            "(title = \"foo\" AND album ~ \"bar\")"
        );
    }

    #[test]
    fn test_add_remove_leaf() {
        let mut state = QueryBuilderState::default();
        // root has 1 leaf by default
        assert_eq!(state.root.clauses.len(), 1);
        state.add_leaf_at(&[]);
        assert_eq!(state.root.clauses.len(), 2);
        // remove first leaf (path = [0])
        state.remove_child_at(&[0]);
        assert_eq!(state.root.clauses.len(), 1);
        // guard: won't remove last child
        state.remove_child_at(&[0]);
        assert_eq!(state.root.clauses.len(), 1);
    }

    #[test]
    fn test_toggle_mode_round_trip() {
        let clause = leaf(Field::Title, Operator::Equal, "hello");
        let query = Query { root: clause };
        let mut state = QueryBuilderState::default();
        state.load_query(&query);
        state.toggle_mode(); // visual → raw
        assert_eq!(state.mode, BuilderMode::RawText);
        state.toggle_mode(); // raw → visual
        assert_eq!(state.mode, BuilderMode::Visual);
        let result = state.try_to_query().unwrap();
        assert_eq!(result.compile_for_storage(), "title = \"hello\"");
    }

    #[test]
    fn test_dropdown_overlay_updates_state() {
        use crate::ui::widgets::overlay::OverlayResult;

        let mut state = QueryBuilderState::default();

        // Navigate down to the first leaf (skip the group header)
        state.cursor.move_down();

        // Get initial state
        let flat = flatten_tree(&state.root);
        assert!(!flat.is_empty());

        let current_node = &flat[state.cursor.flat_index];

        // Get the leaf at cursor position (before)
        let leaf = state
            .leaf_at_mut(&current_node.path)
            .expect("Should be a leaf");
        let initial_field = leaf.field_dd.selected().unwrap().to_string();

        // Get the field options to select a different one
        let field_options = Field::iter().map(|f| f.to_string()).collect::<Vec<_>>();

        // Find a different field to select
        let target_field = field_options
            .iter()
            .find(|f| f.as_str() != initial_field.as_str())
            .unwrap();
        let target_index = field_options
            .iter()
            .position(|f| f == target_field)
            .unwrap();

        // Simulate overlay result - field dropdown is control_id = 1
        let result = OverlayResult::DropdownSelected {
            target_id: 1,
            selected_index: target_index,
        };

        // Apply the overlay result
        let applied = state.apply_overlay_result(&result);
        assert!(applied, "apply_overlay_result should return true");

        // Check that the state was updated
        let flat = flatten_tree(&state.root);
        let current_node = &flat[state.cursor.flat_index];
        let leaf_after = state.leaf_at_mut(&current_node.path).unwrap();
        let new_field = leaf_after.field_dd.selected().unwrap();

        assert_eq!(
            new_field,
            target_field.as_str(),
            "Field should be updated to selected value"
        );

        // Check that the overlay is now closed
        assert!(
            !leaf_after.field_dd.is_open(),
            "Dropdown should be closed after overlay result is applied"
        );
    }

    #[test]
    fn test_visual_edits_sync_to_raw_text() {
        // Test that adding clauses in visual mode syncs to raw text
        let mut state = QueryBuilderState::default();

        // Initial state: visual mode, 1 leaf clause
        assert_eq!(state.mode, BuilderMode::Visual);
        assert_eq!(state.root.clauses.len(), 1);

        // Add another leaf at root level
        state.add_leaf_at(&[]);
        assert_eq!(state.root.clauses.len(), 2);

        // After adding a leaf, both leaves are present but incomplete (no values set)
        // So the tree is invalid and raw_input_valid should be false
        assert!(
            !state.raw_input_valid,
            "Added incomplete leaf makes tree invalid"
        );
    }

    #[test]
    fn test_visual_edits_sync_to_raw_on_removal() {
        let mut state = QueryBuilderState::default();

        // Add a leaf, then remove it
        state.add_leaf_at(&[]);
        assert_eq!(state.root.clauses.len(), 2);

        state.remove_child_at(&[0]);
        assert_eq!(state.root.clauses.len(), 1);

        // After removing a leaf, we're back to the default valid state
        // The raw text should match the visual tree
        if state.raw_input_valid {
            let raw_query =
                Query::from_str(state.raw_input.text()).expect("Raw text should be valid");
            let visual_query = state.try_to_query().expect("Visual tree should compile");

            assert_eq!(
                raw_query.compile_for_storage(),
                visual_query.compile_for_storage(),
                "Raw text should match compiled visual tree after remove_child_at"
            );
        }
    }

    #[test]
    fn test_visual_to_raw_mode_shows_synced_text() {
        // Create a valid query to start with
        let query = Query {
            root: leaf(Field::Title, Operator::Equal, "foo"),
        };
        let mut state = QueryBuilderState::default();
        state.load_query(&query);

        // Start in visual mode
        assert_eq!(state.mode, BuilderMode::Visual);
        assert!(state.raw_input_valid);

        let expected_visual_query = state.try_to_query().unwrap();

        // Switch to raw mode
        state.toggle_mode();
        assert_eq!(state.mode, BuilderMode::RawText);

        // Parse the raw text to verify it matches the visual tree
        let raw_query = Query::from_str(state.raw_input.text()).expect("Raw text should be valid");

        assert_eq!(
            raw_query.compile_for_storage(),
            expected_visual_query.compile_for_storage(),
            "Switching to raw mode should show the synced text from visual state"
        );
    }

    #[test]
    fn test_add_group_syncs_to_raw() {
        let mut state = QueryBuilderState::default();

        // Add a subgroup
        state.add_group_at(&[]);
        assert_eq!(state.root.clauses.len(), 2);

        // After adding a subgroup, the tree is still invalid (both original leaf and new group)
        // because we have an incomplete state
        assert!(
            !state.raw_input_valid,
            "Added group with unmatched leaf makes tree invalid"
        );
    }

    #[test]
    fn test_dropdown_change_syncs_to_raw() {
        use crate::ui::widgets::overlay::OverlayResult;

        // Start with a valid query
        let query = Query {
            root: leaf(Field::Title, Operator::Equal, "foo"),
        };
        let mut state = QueryBuilderState::default();
        state.load_query(&query);

        // Navigate to first leaf
        state.cursor.move_down();

        // Change field via dropdown overlay
        let field_options = Field::iter().map(|f| f.to_string()).collect::<Vec<_>>();
        let target_index = field_options.len() - 1; // select last field

        let result = OverlayResult::DropdownSelected {
            target_id: 1,
            selected_index: target_index,
        };

        state.apply_overlay_result(&result);

        // Raw text should be in sync and valid (we only changed the field, didn't add/remove)
        assert!(
            state.raw_input_valid,
            "Changing field should keep tree valid"
        );

        let raw_query = Query::from_str(state.raw_input.text()).expect("Raw text should be valid");
        let visual_query = state.try_to_query().expect("Visual tree should compile");

        assert_eq!(
            raw_query.compile_for_storage(),
            visual_query.compile_for_storage(),
            "Raw text should match compiled visual tree after dropdown change"
        );
    }
}
