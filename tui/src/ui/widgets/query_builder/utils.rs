use std::{fmt::Display, str::FromStr};

use mecomp_storage::db::schemas::dynamic::query::{
    Clause, Compile as _, CompoundClause, CompoundKind, Field, LeafClause, Operator, Value,
};
use strum::IntoEnumIterator;

use crate::ui::widgets::{
    dropdown::DropdownState,
    overlay::{
        OverlayType,
        set_editor::SetEditorOverlay,
        text::{TextOverlay, ValueKind},
    },
};

/// Gives the parent path of the given path
pub(super) const fn parent_path_of(path: &[usize]) -> &[usize] {
    path.split_at(path.len().saturating_sub(1)).0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiCompoundKind(pub CompoundKind);
impl Display for UiCompoundKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CompoundKind::And => write!(f, "AND"),
            CompoundKind::Or => write!(f, "OR"),
        }
    }
}

/// Macro to build a static array by concatenating multiple other
/// static arrays. Usage: `array_concat!(arr1, arr2, arr3)`.
macro_rules! array_concat {
    ($filler:expr, $($s:expr),+) => {{
        const LEN: usize = $( $s.len() + )* 0;
        & {
            let mut arr = [$filler; LEN];
            let mut base: usize = 0;
            $({
                let mut i = 0;
                while i < $s.len() {
                    arr[base + i] = $s[i];
                    i += 1;
                }
                base += $s.len();
            })*
            if base != LEN {
                panic!("array_concat: length mismatch");
            }
            arr
        }
    }}
}

const SCALAR_SCALAR_OPS: &[Operator] = &[
    Operator::Equal,
    Operator::NotEqual,
    Operator::Like,
    Operator::NotLike,
    Operator::LessThan,
    Operator::LessThanOrEqual,
    Operator::GreaterThan,
    Operator::GreaterThanOrEqual,
];
// additional ops for when both scalars are strings
const STRING_COMPARISON_OPS: &[Operator] = &[
    Operator::Contains,
    Operator::ContainsNot,
    // the below are also technically allowed,
    // but collide with SCALAR_SET_OPS in a way
    // that makes it ambiguous which RHS value is intended.
    // Operator::Inside,
    // Operator::NotInside,
    // Operator::In,
    // Operator::NotIn,
];
const SCALAR_SET_OPS: &[Operator] = &[
    Operator::Inside,
    Operator::NotInside,
    Operator::In,
    Operator::NotIn,
];
const SET_SCALAR_OPS: &[Operator] = &[
    Operator::Contains,
    Operator::ContainsNot,
    Operator::AnyEqual,
    Operator::AllEqual,
    Operator::AnyLike,
    Operator::AllLike,
];
const SET_SET_OPS: &[Operator] = &[
    // Operator::Contains, technically valid, but causes ambiguity with SET_SCALAR_OPS
    Operator::ContainsAll,
    Operator::ContainsAny,
    Operator::ContainsNone,
    Operator::AllInside,
    Operator::AnyInside,
    Operator::NoneInside,
];

const INT_SCALAR_OPS: &[Operator] =
    array_concat!(Operator::Equal, SCALAR_SCALAR_OPS, SCALAR_SET_OPS);
const STRING_SCALAR_OPS: &[Operator] = array_concat!(
    Operator::Equal,
    SCALAR_SCALAR_OPS,
    STRING_COMPARISON_OPS,
    SCALAR_SET_OPS
);
const STRING_SET_OPS: &[Operator] = array_concat!(Operator::Contains, SET_SCALAR_OPS, SET_SET_OPS);

/// Returns the valid operators for a given field, given whether the right-hand value is a set.
#[must_use]
pub const fn operators_for_field(field: Field) -> &'static [Operator] {
    match field {
        // Album and Title are string scalars
        Field::Title | Field::Album => STRING_SCALAR_OPS,
        // ReleaseYear is an integer scalar
        Field::ReleaseYear => INT_SCALAR_OPS,
        // Artists, Genre, and AlbumArtists are sets of strings
        Field::Artists | Field::AlbumArtists | Field::Genre => STRING_SET_OPS,
    }
}

/// Returns the default value for the given (Field, Operator) pair
pub fn default_value_for_field_operator(field: Field, operator: Operator) -> UiValue {
    const STRING_STRING_OPS: &[Operator] =
        array_concat!(Operator::Equal, SCALAR_SCALAR_OPS, STRING_COMPARISON_OPS);

    let default_text = UiValue::Text("value".to_string());
    let default_integer = UiValue::Integer("2000".to_string());
    let default_set = UiValue::Set(vec!["value1".to_string(), "value2".to_string()]);
    let default_integer_set = UiValue::Set(vec!["2000".to_string(), "2001".to_string()]);

    match field {
        Field::Title | Field::Album if STRING_STRING_OPS.contains(&operator) => default_text,
        Field::Title | Field::Album if SCALAR_SET_OPS.contains(&operator) => default_set,
        Field::ReleaseYear if SCALAR_SCALAR_OPS.contains(&operator) => default_integer,
        Field::ReleaseYear if SCALAR_SET_OPS.contains(&operator) => default_integer_set,
        Field::Artists | Field::AlbumArtists | Field::Genre if SET_SET_OPS.contains(&operator) => {
            default_set
        }
        Field::Artists | Field::AlbumArtists | Field::Genre
            if SET_SCALAR_OPS.contains(&operator) =>
        {
            default_text
        }
        _ => {
            panic!("Unhandled (Field, Operator) pair: ({field:?}, {operator:?})");
        }
    }
}

/// The right-hand value of a leaf clause, as editable interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiValue {
    /// Plain text – for Title, Album, and set values when a single string is needed.
    Text(String),
    /// Integer – for `ReleaseYear`.
    Integer(String),
    /// Set of strings – for operators that expect an array right-hand side.
    Set(Vec<String>),
    // TODO: A field reference (e.g. `title = album`), which is a dropdown of available fields.
}

impl Default for UiValue {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl UiValue {
    #[must_use]
    pub const fn is_set(&self) -> bool {
        matches!(self, Self::Set { .. })
    }

    /// Convert to a storage `Value`.  Returns `None` if the input is empty / invalid.
    #[must_use]
    pub fn to_storage_value(&self) -> Option<Value> {
        match self {
            Self::Text(s) => {
                if s.is_empty() {
                    None
                } else {
                    Some(Value::String(s.clone()))
                }
            }
            Self::Integer(s) => s.trim().parse::<i64>().ok().map(Value::Int),
            Self::Set(items) => {
                if items.is_empty() {
                    None
                } else {
                    let values = items
                        .iter()
                        .map(|i| Value::String(i.clone()))
                        .collect::<Vec<_>>();
                    Some(Value::Set(values))
                }
            }
        }
    }

    /// Write an existing `Value` into this `UiValue`, choosing the matching variant.
    pub fn load_value(&mut self, value: &Value) {
        match &value {
            &Value::String(s) => {
                *self = Self::Text(s.clone());
            }
            &Value::Int(i) => {
                *self = Self::Integer(i.to_string());
            }
            &Value::Set(items) => {
                let strings: Vec<String> = items
                    .iter()
                    .filter_map(|v| {
                        if let Value::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                *self = Self::Set(strings);
            }
            Value::Field(f) => {
                // Field references on the right are treated as text
                *self = Self::Text(f.to_string());
            }
        }
    }

    /// Open a text overlay for editing the value.
    #[must_use]
    pub fn open_overlay(&self, control_id: u64) -> OverlayType {
        match self {
            Self::Text(input) => {
                let overlay = TextOverlay::new(control_id, input, ValueKind::Text, 40);
                OverlayType::Text(overlay)
            }
            Self::Integer(input) => {
                let overlay = TextOverlay::new(control_id, input, ValueKind::Integer, 15);
                OverlayType::Text(overlay)
            }
            Self::Set(items) => {
                let overlay = SetEditorOverlay::new(control_id, items.clone(), 50);
                OverlayType::SetEditor(overlay)
            }
        }
    }
}

/// UI state for a single filter row: `field  operator  value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLeafClause {
    pub field_dd: DropdownState<Field>,
    pub operator_dd: DropdownState<&'static Operator>,
    pub value: UiValue,
    /// Which sub-part of the leaf is currently focused (for Tab cycling).
    pub leaf_focus: LeafFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LeafFocus {
    #[default]
    Field,
    Operator,
    Value,
}

impl LeafFocus {
    pub const fn next(self) -> Self {
        match self {
            Self::Field => Self::Operator,
            Self::Operator => Self::Value,
            Self::Value => Self::Field,
        }
    }
    pub const fn prev(self) -> Self {
        match self {
            Self::Field => Self::Value,
            Self::Operator => Self::Field,
            Self::Value => Self::Operator,
        }
    }
}

impl UiLeafClause {
    /// Create a new, blank leaf clause defaulting to `title = "value"`.
    #[must_use]
    pub fn new() -> Self {
        let field = Field::Title;
        let operators = operators_for_field(field);
        let value = default_value_for_field_operator(field, operators[0]);
        Self {
            field_dd: DropdownState::new(1, Field::iter()),
            operator_dd: DropdownState::new(2, operators),
            value,
            leaf_focus: LeafFocus::default(),
        }
    }

    /// The currently selected `Field`.
    #[must_use]
    pub fn field(&self) -> Field {
        let Some(Ok(selected)) = self.field_dd.selected().map(Field::from_str) else {
            panic!("field dropdown has no or invalid selection");
        };
        selected
    }

    /// The currently selected `Operator`.
    #[must_use]
    pub fn operator(&self) -> Option<Operator> {
        Operator::from_str(self.operator_dd.selected()?).ok()
    }

    /// Rebuild operator options after the field (or value type) changes.
    pub fn refresh_operators_and_value(&mut self) {
        // refresh the operator options based on the current field
        let field = self.field();
        let ops = operators_for_field(field);
        self.operator_dd = DropdownState::new(self.operator_dd.control_id(), ops);
        // refresh the value based on the new operator options
        self.refresh_value();
    }

    /// Set the appropriate `UiValue` variant for the current field + operator combo
    pub fn refresh_value(&mut self) {
        let field = self.field();
        let op = self
            .operator()
            .unwrap_or_else(|| operators_for_field(field)[0]);

        self.value = default_value_for_field_operator(field, op);
    }

    /// Try to compile to a storage `LeafClause`.
    #[must_use]
    fn try_to_storage(&self) -> Option<Clause> {
        let field = self.field();
        let op = self.operator()?;
        let right = self.value.to_storage_value()?;
        let left = Value::Field(field);
        let leaf = LeafClause {
            left,
            operator: op,
            right,
        };
        leaf.has_valid_operator().then_some(Clause::Leaf(leaf))
    }

    /// Load from a storage `LeafClause`.
    pub fn load_leaf(&mut self, leaf: &LeafClause) {
        // Field (from left side)
        if let Value::Field(f) = leaf.left {
            let _ = self
                .field_dd
                .select_by_text(f.compile_for_storage().as_str());
        }
        // Rebuild operators for that field
        let field = self.field();
        let ops = operators_for_field(field);
        self.operator_dd = DropdownState::new(self.operator_dd.control_id(), ops);
        let _ = self
            .operator_dd
            .select_by_text(leaf.operator.compile_for_storage().as_str());
        // Value
        self.value.load_value(&leaf.right);
    }
}

/// A clause in the UI query tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiClause {
    Leaf(UiLeafClause),
    Group(UiGroup),
}

impl UiClause {
    /// Try to compile this group to a storage `Clause`.
    #[must_use]
    pub fn try_to_storage(&self) -> Option<Clause> {
        match self {
            Self::Group(g) => g.try_to_storage(),
            Self::Leaf(l) => l.try_to_storage(),
        }
    }

    #[must_use]
    pub const fn leaf(&mut self) -> Option<&mut UiLeafClause> {
        if let Self::Leaf(leaf) = self {
            Some(leaf)
        } else {
            None
        }
    }
    #[must_use]
    pub const fn group(&mut self) -> Option<&mut UiGroup> {
        if let Self::Group(group) = self {
            Some(group)
        } else {
            None
        }
    }
}

/// An N-ary group (AND / OR) of clauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiGroup {
    /// The AND/OR selector.
    pub kind_dd: DropdownState<UiCompoundKind>,
    /// Child clauses (may be < 2 while the user is still building).
    pub clauses: Vec<UiClause>,
}

impl UiGroup {
    #[must_use]
    pub fn new(kind: CompoundKind) -> Self {
        let mut kind_dd = DropdownState::new(0, CompoundKind::iter().map(UiCompoundKind));
        let _ = kind_dd.select_by_text(UiCompoundKind(kind).to_string().as_str());

        Self {
            kind_dd,
            clauses: vec![
                UiClause::Leaf(UiLeafClause::new()),
                UiClause::Leaf(UiLeafClause::new()),
            ],
        }
    }

    #[must_use]
    pub fn kind(&self) -> CompoundKind {
        let Some(selected) = self.kind_dd.selected() else {
            return CompoundKind::And;
        };
        match selected {
            "OR" => CompoundKind::Or,
            _ => CompoundKind::And,
        }
    }

    /// Try to compile this group to a storage `Clause`.
    #[must_use]
    fn try_to_storage(&self) -> Option<Clause> {
        let children: Vec<Clause> = self
            .clauses
            .iter()
            .filter_map(UiClause::try_to_storage)
            .collect();

        match children.len() {
            0 => None,
            1 => Some(children.into_iter().next().unwrap()),
            _ => {
                // Left-fold into a binary tree: (((a AND b) AND c) AND d)
                let kind = self.kind();
                let result = children
                    .into_iter()
                    .reduce(|acc, next| {
                        Clause::Compound(CompoundClause {
                            clauses: vec![acc, next],
                            kind,
                        })
                    })
                    .unwrap(); // safe: len >= 2
                Some(result)
            }
        }
    }
}

/// A node in the flat DFS traversal of the UI clause tree, used for rendering and cursor
/// navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatNodeKind {
    /// A group header row (shows AND/OR selector + add/del buttons).
    GroupHeader,
    /// A leaf row (field + operator + value + del button).
    Leaf,
    /// The `[+ Add Clause]` button at the end of a group's children.
    AddClause,
    /// The `[+ Add Group]` button at the end of a group's children.
    AddGroup,
}

/// A flat node produced by DFS tree traversal.
#[derive(Debug, Clone)]
pub struct FlatNode {
    /// Visual depth (for indentation).
    pub depth: usize,
    /// Path into the tree: each element is the child index at that level.
    /// Empty path = the root group.
    pub path: Vec<usize>,
    pub kind: FlatNodeKind,
}

/// Flatten a `UiClause` tree into an ordered, displayable list of [`FlatNode`]s.
pub fn flatten_tree(root: &UiClause) -> Vec<FlatNode> {
    let mut out = Vec::new();
    match root {
        UiClause::Leaf(_) => {
            out.push(FlatNode {
                depth: 0,
                path: vec![],
                kind: FlatNodeKind::Leaf,
            });
            out.push(FlatNode {
                depth: 0,
                path: vec![usize::MAX - 1], // sentinel: add clause button
                kind: FlatNodeKind::AddClause,
            });
            out.push(FlatNode {
                depth: 0,
                path: vec![usize::MAX], // sentinel: add group button
                kind: FlatNodeKind::AddGroup,
            });
        }
        UiClause::Group(group) => {
            flatten_group(group, &[], 0, &mut out);
        }
    }
    out
}

fn flatten_group(group: &UiGroup, path: &[usize], depth: usize, out: &mut Vec<FlatNode>) {
    // The group header itself
    out.push(FlatNode {
        depth,
        path: path.to_vec(),
        kind: FlatNodeKind::GroupHeader,
    });

    // Children
    for (i, child) in group.clauses.iter().enumerate() {
        let mut child_path = path.to_vec();
        child_path.push(i);
        match child {
            UiClause::Leaf(_) => {
                out.push(FlatNode {
                    depth: depth + 1,
                    path: child_path,
                    kind: FlatNodeKind::Leaf,
                });
            }
            UiClause::Group(sub) => {
                flatten_group(sub, &child_path, depth + 1, out);
            }
        }
    }

    // Add-buttons at the end of this group
    // let mut add_path = path.to_vec();
    // add_path.push(usize::MAX); // sentinel: add button
    out.push(FlatNode {
        depth: depth + 1,
        path: {
            let mut p = path.to_vec();
            p.push(usize::MAX - 1);
            p
        },
        kind: FlatNodeKind::AddClause,
    });
    out.push(FlatNode {
        depth: depth + 1,
        path: {
            let mut p = path.to_vec();
            p.push(usize::MAX);
            p
        },
        kind: FlatNodeKind::AddGroup,
    });
}

/// Navigates the flat node list by flat index.
#[derive(Debug, Clone)]
pub struct CursorPath {
    /// Index into the flat list returned by `flatten_tree`.
    pub flat_index: usize,
    /// Cached length of the flat list
    cached_len: usize,
    /// Cached scroll offset for rendering
    cached_scroll: usize,
}

impl CursorPath {
    pub const fn new(len: usize) -> Self {
        Self {
            flat_index: 0,
            cached_len: len,
            cached_scroll: 0,
        }
    }

    pub const fn move_up(&mut self) {
        if self.flat_index == 0 {
            self.flat_index = self.cached_len.saturating_sub(1);
        } else {
            self.flat_index -= 1;
        }
    }

    pub const fn move_down(&mut self) {
        if self.cached_len == 0 {
            return;
        }
        self.flat_index = (self.flat_index + 1) % self.cached_len;
    }

    // Should be called at least after any tree modification.
    /// Update the cached flat list length and clamp the index if needed.
    pub const fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.flat_index = 0;
        } else if self.flat_index >= len {
            self.flat_index = len - 1;
        }
        self.cached_len = len;
    }

    /// Compute and update the correct scroll offset for rendering, given the number of visible rows.
    pub const fn scroll_offset(&mut self, visible: usize) -> usize {
        // based on the provided visible row count, adjust the scroll offset

        // if cursor is above the visible area, scroll up to show it
        if self.flat_index < self.cached_scroll {
            self.cached_scroll = self.flat_index;
        }
        // if the cursor is below the visible area, scroll down to show it
        if self.flat_index >= self.cached_scroll + visible {
            self.cached_scroll = self.flat_index + 1 - visible;
        }
        self.cached_scroll
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};
    use rstest::rstest;

    use crate::ui::widgets::query_builder::QueryBuilderState;

    use super::*;

    #[test]
    fn test_uigroup_to_query() {
        let mut clause1 = UiLeafClause::new();
        clause1.load_leaf(&LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::String("foo".to_string()),
        });
        let clause1 = UiClause::Leaf(clause1);

        let mut clause2 = UiLeafClause::new();
        clause2.load_leaf(&LeafClause {
            left: Value::Field(Field::Album),
            operator: Operator::Like,
            right: Value::String("bar".to_string()),
        });
        let clause2 = UiClause::Leaf(clause2);

        let mut group = UiGroup::new(CompoundKind::And);
        group.clauses = vec![clause1, clause2];

        // try converting to a query
        let query = group
            .try_to_storage()
            .expect("couldn't convert UiClause::Group to Clause");
        let query = query.compile_for_storage();
        assert_str_eq!(query, "(title = \"foo\" AND album ~ \"bar\")")
    }

    #[test]
    fn test_flatten_tree() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);
        let nodes = flatten_tree(&state.root);
        // root header + 2 leaves + AddClause + AddGroup
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[0].kind, FlatNodeKind::GroupHeader);
        assert_eq!(nodes[1].kind, FlatNodeKind::Leaf);
        assert_eq!(nodes[2].kind, FlatNodeKind::Leaf);
        assert_eq!(nodes[3].kind, FlatNodeKind::AddClause);
        assert_eq!(nodes[4].kind, FlatNodeKind::AddGroup);
    }

    #[test]
    fn test_operators_for_scalar_field() {
        let ops = operators_for_field(Field::Title);
        assert!(ops.iter().any(|o| *o == Operator::Equal));
        assert!(ops.iter().any(|o| *o == Operator::Like));
    }

    #[test]
    fn test_operators_for_set_field() {
        let ops = operators_for_field(Field::Artists);
        assert!(ops.iter().any(|o| *o == Operator::Contains));
        // scalar-only operators should not appear
        assert!(!ops.iter().any(|o| *o == Operator::Equal));
    }

    #[test]
    fn parent_path_of_returns_parent_path() {
        assert_eq!(parent_path_of(&[0, 1, 2]), &[0, 1]);
        assert_eq!(parent_path_of(&[0]), &[] as &[usize]);
        assert_eq!(parent_path_of(&[]), &[] as &[usize]);
    }

    #[test]
    fn refresh_value_sets_integer_for_release_year() {
        let mut leaf = UiLeafClause::new();
        let _ = leaf
            .field_dd
            .select_by_text(Field::ReleaseYear.compile_for_storage().as_str());
        let _ = leaf
            .operator_dd
            .select_by_text(Operator::Equal.compile_for_storage().as_str());

        leaf.refresh_value();

        assert_eq!(leaf.value, UiValue::Integer("2000".to_string()));
    }

    #[test]
    fn refresh_value_sets_set_when_operator_requires_set() {
        let mut leaf = UiLeafClause::new();
        leaf.field_dd = DropdownState::new(1, vec![Field::Title]);
        leaf.operator_dd = DropdownState::new(2, vec![&Operator::In]);

        leaf.refresh_value();

        assert!(matches!(leaf.value, UiValue::Set(_)));
    }

    #[rstest]
    #[case(Field::Title, STRING_SCALAR_OPS, 14)]
    #[case(Field::Album, STRING_SCALAR_OPS, 14)]
    #[case(Field::ReleaseYear, INT_SCALAR_OPS, 12)]
    #[case(Field::Artists, STRING_SET_OPS, 12)]
    #[case(Field::AlbumArtists, STRING_SET_OPS, 12)]
    #[case(Field::Genre, STRING_SET_OPS, 12)]
    fn test_operators_for_field(
        #[case] field: Field,
        #[case] expected_ops: &[Operator],
        #[case] expected_len: usize,
    ) {
        let ops = operators_for_field(field);
        assert_eq!(ops, expected_ops);
        assert_eq!(ops.len(), expected_len);

        // ensure that no duplicate operators are present
        let mut seen = std::collections::HashSet::new();
        for &op in ops {
            assert!(seen.insert(op), "Duplicate operator found: {:?}", op);
        }
    }

    #[rstest]
    #[case(&[Field::ReleaseYear], 0, 8, UiValue::Integer("2000".to_string()))]
    #[case(&[Field::ReleaseYear], 8, 12, UiValue::Set(vec!["2000".to_string(), "2001".to_string()]))]
    #[case(&[Field::Title, Field::Album], 0, 10, UiValue::Text("value".to_string()))]
    #[case(&[Field::Title, Field::Album], 10, 14, UiValue::Set(vec!["value1".to_string(), "value2".to_string()]))]
    #[case(&[Field::Artists, Field::AlbumArtists, Field::Genre], 0, 6, UiValue::Text("value".to_string()))]
    #[case(&[Field::Artists, Field::AlbumArtists, Field::Genre], 6, 12, UiValue::Set(vec!["value1".to_string(), "value2".to_string()]))]
    fn test_default_values_for_field_operator(
        #[case] fields: &[Field],
        #[case] start: usize,
        #[case] end: usize,
        #[case] expected_value: UiValue,
    ) {
        for &field in fields {
            let ops = operators_for_field(field);
            let should_match = &ops[start..end];
            let shouldnt_match = ops[..start].iter().chain(&ops[end..]);

            for &op in should_match {
                assert_eq!(
                    default_value_for_field_operator(field, op),
                    expected_value,
                    "Field: {:?}, Operator: {:?}",
                    field,
                    op
                );
            }
            for &op in shouldnt_match {
                assert_ne!(
                    default_value_for_field_operator(field, op),
                    expected_value,
                    "Field: {:?}, Operator: {:?}",
                    field,
                    op
                );
            }
        }
    }
}
