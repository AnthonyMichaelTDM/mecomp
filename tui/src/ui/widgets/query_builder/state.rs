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
    query_builder::utils::{LeafFocus, UiValue, flatten_tree, parent_path_of},
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
    // TODO: use this instead of hardcoding target_id values in apply_overlay_result
    // pub fn id(&self) -> u8 {
    //     match self {
    //         ClickableAction::GroupKind => 0,
    //         ClickableAction::LeafField => 1,
    //         ClickableAction::LeafOperator => 2,
    //         ClickableAction::LeafValue => 3,
    //         ClickableAction::Delete => 4,
    //         ClickableAction::AddClause => 5,
    //         ClickableAction::AddGroup => 6,
    //     }
    // }
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
    /// The root group.
    pub root: UiClause,
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
            root: UiClause::Leaf(UiLeafClause::new()),
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
                    let Some(UiClause::Group(group)) = self.clause_at_mut(&current_node.path)
                    else {
                        return false;
                    };
                    group.kind_dd.apply_overlay_result(result)
                } else if *target_id > 0 && *target_id <= 2 {
                    // this is a dropdown for a leaf clause
                    let Some(UiClause::Leaf(leaf)) = self.clause_at_mut(&current_node.path) else {
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
                    && let Some(UiClause::Leaf(leaf)) = self.clause_at_mut(&current_node.path)
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
                    && let Some(UiClause::Leaf(leaf)) = self.clause_at_mut(&current_node.path)
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
            BuilderMode::Visual => self.root.try_to_storage().map(|root| Query { root }),
        }
    }

    /// Load a `Query` into the visual builder.
    pub fn load_query(&mut self, query: &Query) {
        self.root = clause_to_ui_clause(&query.root);
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
                    self.root = clause_to_ui_clause(&q.root);
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
        if let Some(q) = self.root.try_to_storage().map(|root| Query { root }) {
            self.raw_input.set_text(&q.compile_for_storage());
            self.raw_input_valid = true;
        } else {
            self.raw_input_valid = false;
        }
    }

    // ── Mutators ─────────────────────────────────────────────────────────────

    /// Add a new leaf clause to the group at `path` (empty path = root).
    ///
    /// If we try to add a leaf to a leaf node, we convert it to a group with the existing leaf and the new leaf as children.
    pub fn add_leaf_at(&mut self, path: &[usize]) {
        let Some(clause) = self.clause_at_mut(path) else {
            return;
        };
        let new_leaf = UiClause::Leaf(UiLeafClause::new());
        match clause {
            UiClause::Group(group) => {
                group.clauses.push(new_leaf);
            }
            UiClause::Leaf(leaf) => {
                // adding a leaf at a leaf -> convert to group
                let new_leaf = UiLeafClause::new();
                *clause = UiClause::Group(UiGroup {
                    kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                    clauses: vec![UiClause::Leaf(leaf.clone()), UiClause::Leaf(new_leaf)],
                });
            }
        }
        self.sync_raw_from_visual();
    }

    /// Add a new sub-group to the group at `path`.
    ///
    /// If adding a group to a leaf node, we convert it to a group with the existing leaf and new group as children, similar to `add_leaf_at`.
    pub fn add_group_at(&mut self, path: &[usize]) {
        let Some(clause) = self.clause_at_mut(path) else {
            return;
        };
        let new_group = UiClause::Group(UiGroup::new(CompoundKind::And));
        match clause {
            UiClause::Group(group) => {
                group.clauses.push(new_group);
            }
            UiClause::Leaf(leaf) => {
                // adding a group at a leaf -> convert to group
                *clause = UiClause::Group(UiGroup {
                    kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                    clauses: vec![UiClause::Leaf(leaf.clone()), new_group],
                });
            }
        }
        self.sync_raw_from_visual();
    }

    /// Remove the child at position `child_index` in the group at `path`.
    ///
    /// If the parent group would have only one child left after removal, we remove the group and promote the remaining child
    /// (if it's a leaf) to take its place, to avoid degenerate 1-child groups.
    pub fn remove_child_at(&mut self, path: &[usize]) {
        if path.is_empty() {
            return; // can't remove root
        }
        // path[-1] is the index of the child to remove; path[..-1] is the parent group path
        let parent_path = parent_path_of(path);
        let child_idx = path.last().copied().unwrap_or_default();

        if let Some(UiClause::Group(parent)) = self.clause_at_mut(parent_path)
            && child_idx < parent.clauses.len()
        {
            // remove the child
            parent.clauses.remove(child_idx);

            // promote remaining child if necessary
            if parent.clauses.len() <= 1 {
                let Some(only_child) = parent.clauses.pop() else {
                    return self.sync_raw_from_visual();
                };
                let grandparent_path = parent_path_of(parent_path);
                if let Some(UiClause::Group(grandparent)) = self.clause_at_mut(grandparent_path)
                    && grandparent_path.len() < parent_path.len()
                {
                    let idx_in_grandparent = parent_path.last().copied().unwrap_or_default();
                    grandparent.clauses[idx_in_grandparent] = only_child;
                } else {
                    // if no grandparent, we're at root - promote to root
                    self.root = only_child;
                }
            }
        }

        self.sync_raw_from_visual();
    }

    /// Returns a mutable reference to the clause at the given path
    pub fn clause_at_mut<'a>(&'a mut self, path: &[usize]) -> Option<&'a mut UiClause> {
        navigate_to_clause_mut(&mut self.root, path)
    }
}

// ── Navigation helpers ───────────────────────────────────────────────────────

fn navigate_to_clause_mut<'a>(root: &'a mut UiClause, path: &[usize]) -> Option<&'a mut UiClause> {
    if path.is_empty() {
        return Some(root);
    }
    let idx = path[0];
    let child = match root {
        UiClause::Group(g) => g.clauses.get_mut(idx)?,
        UiClause::Leaf(_) => return None,
    };
    navigate_to_clause_mut(child, &path[1..])
}

// ── Conversion from storage types ───────────────────────────────────────────

/// Convert a storage `Clause` to a `UiGroup`.
///
/// - A `Compound` becomes a proper `UiGroup` with N-arified children.
/// - A bare `Leaf` becomes a `UiGroup` wrapping a single leaf.
fn clause_to_ui_clause(clause: &Clause) -> UiClause {
    match clause {
        Clause::Compound(c) => compound_to_ui_clause(c),
        Clause::Leaf(l) => {
            let mut ui_leaf = UiLeafClause::new();
            ui_leaf.load_leaf(l);
            UiClause::Leaf(ui_leaf)
        }
    }
}

fn compound_to_ui_clause(compound: &CompoundClause) -> UiClause {
    let kind = compound.kind;
    let mut kind_dd = DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind));
    let _ = kind_dd.select_by_text(UiCompoundKind(kind).to_string().as_str());

    // Flatten N-ary: if nested compounds have the same kind, absorb their children.
    let mut children: Vec<UiClause> = Vec::new();
    flatten_compound_children(&compound.clauses, kind, &mut children);

    UiClause::Group(UiGroup {
        kind_dd,
        clauses: children,
    })
}

fn flatten_compound_children(clauses: &[Clause], kind: CompoundKind, out: &mut Vec<UiClause>) {
    for clause in clauses {
        match clause {
            Clause::Compound(c) if c.kind == kind => {
                // absorb children of same-kind compound
                flatten_compound_children(&c.clauses, kind, out);
            }
            Clause::Compound(c) => {
                out.push(compound_to_ui_clause(c));
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
    fn test_load_query() {
        let mut state = QueryBuilderState::default();
        let query = Query {
            root: leaf(Field::Title, Operator::Equal, "foo"),
        };
        state.load_query(&query);
        assert_eq!(
            state.mode,
            BuilderMode::Visual,
            "should reset to visual mode"
        );
        assert_eq!(
            state.raw_input.text(),
            "title = \"foo\"",
            "raw input should be set to compiled query"
        );
        assert!(
            state.raw_input_valid,
            "raw input should be valid since query is valid"
        );
        let mut expected = UiLeafClause::new();
        expected.value = UiValue::Text("foo".to_string());
        assert_eq!(
            state.root,
            UiClause::Leaf(expected),
            "root should be a leaf clause with correct field/operator/value"
        );
    }

    #[test]
    fn test_navigate_to_clause() {
        let mut state = QueryBuilderState::default();
        state.add_group_at(&[]); // root is now a group with 2 children
        state.add_leaf_at(&[0]); // first child is a group with 2 leafs
        state.add_leaf_at(&[1]); // second child is a group with 2 leafs

        // so, overall, the tree should look like this:
        // root (group)
        // |-- child 0 (group)
        // |   |-- child 0 (leaf)
        // |   |-- child 1 (leaf)
        // |-- child 1 (group)
        //     |-- child 0 (leaf)
        //     |-- child 1 (leaf)

        // navigate to each leaf/group and check we get the expected clause
        let clause = state.clause_at_mut(&[]).unwrap();
        assert!(matches!(clause, UiClause::Group(_)));
        let clause = state.clause_at_mut(&[0]).unwrap();
        assert!(matches!(clause, UiClause::Group(_)));
        let clause = state.clause_at_mut(&[0, 0]).unwrap();
        assert!(matches!(clause, UiClause::Leaf(_)));
        let clause = state.clause_at_mut(&[0, 1]).unwrap();
        assert!(matches!(clause, UiClause::Leaf(_)));
        let clause = state.clause_at_mut(&[1]).unwrap();
        assert!(matches!(clause, UiClause::Group(_)));
        let clause = state.clause_at_mut(&[1, 0]).unwrap();
        assert!(matches!(clause, UiClause::Leaf(_)));
        let clause = state.clause_at_mut(&[1, 1]).unwrap();
        assert!(matches!(clause, UiClause::Leaf(_)));

        // invalid paths returns None
        assert!(state.clause_at_mut(&[2]).is_none());
        assert!(state.clause_at_mut(&[0, 2]).is_none());
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
        assert_eq!(state.root, UiClause::Leaf(UiLeafClause::new()));
        state.add_leaf_at(&[]);
        assert_eq!(
            state.root,
            UiClause::Group(UiGroup {
                kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                clauses: vec![
                    UiClause::Leaf(UiLeafClause::new()),
                    UiClause::Leaf(UiLeafClause::new())
                ]
            })
        );
        // remove first leaf (path = [0])
        state.remove_child_at(&[0]);
        assert_eq!(state.root, UiClause::Leaf(UiLeafClause::new()));
        // guard: won't remove last child
        state.remove_child_at(&[]);
        assert_eq!(state.root, UiClause::Leaf(UiLeafClause::new()));
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

        // Get initial state
        let flat = flatten_tree(&state.root);
        assert!(!flat.is_empty());

        // Get the leaf at cursor position (before)
        let leaf = match &state.root {
            UiClause::Leaf(leaf) => leaf,
            _ => panic!("Expected root to be a leaf clause"),
        };
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
        let leaf_after = state
            .clause_at_mut(&current_node.path)
            .and_then(|c| match c {
                UiClause::Leaf(leaf) => Some(leaf),
                _ => None,
            })
            .unwrap();
        let new_field = leaf_after.field_dd.selected().unwrap();

        assert_eq!(
            new_field,
            target_field.as_str(),
            "Field should be updated to selected value"
        );
    }

    #[test]
    fn test_visual_edits_sync_to_raw_text() {
        // Test that adding clauses in visual mode syncs to raw text
        let mut state = QueryBuilderState::default();

        // Initial state: visual mode, 1 leaf clause
        assert_eq!(state.mode, BuilderMode::Visual);
        assert_eq!(state.root, UiClause::Leaf(UiLeafClause::new()));

        // Add another leaf at root level
        state.add_leaf_at(&[]);
        assert_eq!(
            state.root,
            UiClause::Group(UiGroup {
                kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                clauses: vec![
                    UiClause::Leaf(UiLeafClause::new()),
                    UiClause::Leaf(UiLeafClause::new())
                ]
            })
        );
        assert!(state.raw_input_valid,);
        assert_eq!(
            state.raw_input.text(),
            "(title = \"value\" OR title = \"value\")"
        );
    }

    #[test]
    fn test_visual_edits_sync_to_raw_on_removal() {
        let mut state = QueryBuilderState::default();

        // Add a leaf, then remove it
        state.add_leaf_at(&[]);

        state.remove_child_at(&[0]);
        assert_eq!(state.root, UiClause::Leaf(UiLeafClause::new()));

        assert!(state.raw_input_valid);
        assert_eq!(state.raw_input.text(), "title = \"value\"");
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
        assert_eq!(
            state.root,
            UiClause::Group(UiGroup {
                kind_dd: DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind)),
                clauses: vec![
                    UiClause::Leaf(UiLeafClause::new()),
                    UiClause::Group(UiGroup::new(CompoundKind::And))
                ]
            })
        );
        assert!(state.raw_input_valid);
        assert_eq!(
            state.raw_input.text(),
            "(title = \"value\" OR (title = \"value\" AND title = \"value\"))",
            "Raw text should reflect added group with empty leaf clauses"
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
