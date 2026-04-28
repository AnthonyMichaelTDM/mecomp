//! UI-side data model for the query builder.

use std::str::FromStr;

use mecomp_storage::db::schemas::dynamic::query::{
    Clause, Compile as _, CompoundClause, CompoundKind, Query,
};
use strum::IntoEnumIterator;

use crate::ui::widgets::{
    dropdown::DropdownState, input_box::InputBoxState, overlay::OverlayResult,
    query_builder::utils::flatten_tree,
};

use super::utils::{CursorPath, UiClause, UiCompoundKind, UiGroup, UiLeafClause};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuilderMode {
    #[default]
    Visual,
    RawText,
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
    // TODO: a way to map some ID into the area on screen where that control is rendered, for mouse click handling.
}

impl Default for QueryBuilderState {
    fn default() -> Self {
        Self {
            mode: BuilderMode::Visual,
            root: UiGroup::new(CompoundKind::And),
            cursor: CursorPath::default(),
            raw_input: InputBoxState::new(),
            raw_input_valid: false,
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
        match result {
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
        }
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
        self.cursor = CursorPath::default();
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
                if let Some(q) = self.root.try_to_clause().map(|root| Query { root }) {
                    self.raw_input.set_text(&q.compile_for_storage());
                    self.raw_input_valid = true;
                } else {
                    self.raw_input_valid = false;
                }
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

    // ── Mutators ─────────────────────────────────────────────────────────────

    /// Add a new leaf clause to the group at `path` (empty path = root).
    pub fn add_leaf_at(&mut self, path: &[usize]) {
        if let Some(group) = self.group_at_mut(path) {
            group.clauses.push(UiClause::Leaf(UiLeafClause::new()));
        }
    }

    /// Add a new sub-group to the group at `path`.
    pub fn add_group_at(&mut self, path: &[usize]) {
        if let Some(group) = self.group_at_mut(path) {
            let new_group = UiGroup::new(CompoundKind::And);
            group.clauses.push(UiClause::Group(new_group));
        }
    }

    /// Remove the child at position `child_index` in the group at `path`.
    /// Guards: won't remove if the parent group would drop below 1 child.
    pub fn remove_child_at(&mut self, path: &[usize]) {
        guard_remove(&mut self.root, path);
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
                kind_dd: DropdownState::new(
                    0,
                    vec![
                        UiCompoundKind(CompoundKind::And),
                        UiCompoundKind(CompoundKind::Or),
                    ],
                ),
                clauses: vec![UiClause::Leaf(ui_leaf)],
            }
        }
    }
}

fn compound_to_ui_group(compound: &CompoundClause) -> UiGroup {
    let kind = compound.kind;
    let kind_idx = match kind {
        CompoundKind::And => 0,
        CompoundKind::Or => 1,
    };
    let mut kind_dd = DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind));
    kind_dd.set_selected_index(kind_idx);

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
        let flat = flatten_tree(&state.root);
        state.cursor.move_down(flat.len());

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
}
