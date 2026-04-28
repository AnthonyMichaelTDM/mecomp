use std::{fmt::Display, str::FromStr};

use mecomp_storage::db::schemas::dynamic::query::{
    Clause, Compile as _, CompoundClause, CompoundKind, Field, LeafClause, Operator, Value,
};
use strum::IntoEnumIterator;

use crate::ui::widgets::{dropdown::DropdownState, input_box::InputBoxState};

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

/// Returns the valid operators for a given field, given whether the right-hand value is a set.
#[must_use]
pub fn operators_for_field(field: Field, right_is_set: bool) -> Vec<Operator> {
    let ops: &[Operator] = match (field_is_set(field), right_is_set) {
        // scalar ↔ scalar
        (false, false) => &[
            Operator::Equal,
            Operator::NotEqual,
            Operator::Like,
            Operator::NotLike,
            Operator::LessThan,
            Operator::LessThanOrEqual,
            Operator::GreaterThan,
            Operator::GreaterThanOrEqual,
            Operator::Contains,
            Operator::ContainsNot,
            Operator::Inside,
            Operator::NotInside,
            Operator::In,
            Operator::NotIn,
        ],
        // scalar → set  (value INSIDE set_field)
        (false, true) => &[
            Operator::Inside,
            Operator::NotInside,
            Operator::In,
            Operator::NotIn,
        ],
        // set field → scalar
        (true, false) => &[
            Operator::Contains,
            Operator::ContainsNot,
            Operator::AnyEqual,
            Operator::AllEqual,
            Operator::AnyLike,
            Operator::AllLike,
        ],
        // set ↔ set
        (true, true) => &[
            Operator::Contains,
            Operator::ContainsAll,
            Operator::ContainsAny,
            Operator::ContainsNone,
            Operator::AllInside,
            Operator::AnyInside,
            Operator::NoneInside,
        ],
    };
    ops.to_vec()
}

/// Returns `true` if `field` holds an array value `(artists, album_artists, genre)`.
#[must_use]
pub const fn field_is_set(field: Field) -> bool {
    matches!(field, Field::Artists | Field::AlbumArtists | Field::Genre)
}

/// The right-hand value of a leaf clause, as editable interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiValue {
    /// Plain text – for Title, Album, and set values when a single string is needed.
    Text(InputBoxState),
    /// Integer – for `ReleaseYear`.
    Integer(InputBoxState),
    /// Set of strings – for operators that expect an array right-hand side.
    Set {
        items: Vec<String>,
        item_input: InputBoxState,
    },
}

impl Default for UiValue {
    fn default() -> Self {
        Self::Text(InputBoxState::new())
    }
}

impl UiValue {
    #[must_use]
    pub const fn is_set(&self) -> bool {
        matches!(self, Self::Set { .. })
    }

    /// Build the appropriate `UiValue` for a given `Field`.
    #[must_use]
    pub fn for_field(field: Field) -> Self {
        if field == Field::ReleaseYear {
            Self::Integer(InputBoxState::new())
        } else {
            Self::Text(InputBoxState::new())
        }
    }

    /// Convert to a storage `Value`.  Returns `None` if the input is empty / invalid.
    #[must_use]
    pub fn to_storage_value(&self) -> Option<Value> {
        match self {
            Self::Text(s) => {
                let t = s.text().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(Value::String(t))
                }
            }
            Self::Integer(s) => {
                let t = s.text();
                if t.is_empty() {
                    None
                } else {
                    s.text().trim().parse::<i64>().ok().map(Value::Int)
                }
            }
            Self::Set { items, .. } => {
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
                let mut input = InputBoxState::new();
                input.set_text(s);
                *self = Self::Text(input);
            }
            &Value::Int(i) => {
                let mut input = InputBoxState::new();
                input.set_text(&i.to_string());
                *self = Self::Integer(input);
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
                *self = Self::Set {
                    items: strings,
                    item_input: InputBoxState::new(),
                };
            }
            Value::Field(_) => {
                // Field references on the right are treated as text
                *self = Self::Text(InputBoxState::new());
            }
        }
    }
}

/// UI state for a single filter row: `field  operator  value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLeafClause {
    pub field_dd: DropdownState<Field>,
    pub operator_dd: DropdownState<Operator>,
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
    /// Create a new, blank leaf clause defaulting to `title = ""`.
    #[must_use]
    pub fn new() -> Self {
        let field = Field::Title;
        let operators = operators_for_field(field, false);
        Self {
            field_dd: DropdownState::new(1, Field::iter()),
            operator_dd: DropdownState::new(2, operators),
            value: UiValue::Text(InputBoxState::new()),
            leaf_focus: LeafFocus::default(),
        }
    }

    /// The currently selected `Field`.
    #[must_use]
    pub fn field(&self) -> Field {
        let Some(selected) = self.field_dd.selected() else {
            return Field::Title;
        };
        Field::from_str(selected).unwrap_or(Field::Title)
    }

    /// The currently selected `Operator`.
    #[must_use]
    pub fn operator(&self) -> Option<Operator> {
        Operator::from_str(self.operator_dd.selected()?).ok()
    }

    /// Rebuild operator options after the field (or value type) changes.
    pub fn refresh_operators(&mut self) {
        let field = self.field();
        let right_is_set = self.value.is_set();
        let ops = operators_for_field(field, right_is_set);
        self.operator_dd = DropdownState::new(self.operator_dd.control_id(), ops);
    }

    /// Called when the selected field changes: resets the value to the appropriate type and
    /// refreshes the operator list.
    pub fn on_field_changed(&mut self) {
        let field = self.field();
        self.value = UiValue::for_field(field);
        self.refresh_operators();
    }

    /// Try to compile to a storage `LeafClause`.
    #[must_use]
    pub fn try_to_leaf(&self) -> Option<LeafClause> {
        let field = self.field();
        let op = self.operator()?;
        let right = self.value.to_storage_value()?;
        let left = Value::Field(field);
        let leaf = LeafClause {
            left,
            operator: op,
            right,
        };
        if leaf.has_valid_operator() {
            Some(leaf)
        } else {
            None
        }
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
        let right_is_set = matches!(leaf.right, Value::Set(_));
        let ops = operators_for_field(field, right_is_set);
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
            clauses: vec![UiClause::Leaf(UiLeafClause::new())],
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
    pub fn try_to_clause(&self) -> Option<Clause> {
        let children: Vec<Clause> = self
            .clauses
            .iter()
            .filter_map(try_clause_to_storage)
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

fn try_clause_to_storage(clause: &UiClause) -> Option<Clause> {
    match clause {
        UiClause::Leaf(l) => l.try_to_leaf().map(Clause::Leaf),
        UiClause::Group(g) => g.try_to_clause(),
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
/// The root is always a group.
pub fn flatten_tree(root: &UiGroup) -> Vec<FlatNode> {
    let mut out = Vec::new();
    flatten_group(root, &[], 0, &mut out);
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
#[derive(Debug, Default, Clone)]
pub struct CursorPath {
    /// Index into the flat list returned by `flatten_tree`.
    pub flat_index: usize,
}

impl CursorPath {
    pub const fn move_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        if self.flat_index == 0 {
            self.flat_index = len - 1;
        } else {
            self.flat_index -= 1;
        }
    }

    pub const fn move_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.flat_index = (self.flat_index + 1) % len;
    }

    pub const fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.flat_index = 0;
        } else if self.flat_index >= len {
            self.flat_index = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;

    use crate::ui::widgets::query_builder::QueryBuilderState;

    use super::*;

    #[test]
    fn test_uigroup_to_qury() {
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
            .try_to_clause()
            .expect("couldn't convert UiGroup to Clause");
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
        let ops = operators_for_field(Field::Title, false);
        assert!(ops.iter().any(|o| *o == Operator::Equal));
        assert!(ops.iter().any(|o| *o == Operator::Like));
    }

    #[test]
    fn test_operators_for_set_field() {
        let ops = operators_for_field(Field::Artists, false);
        assert!(ops.iter().any(|o| *o == Operator::Contains));
        // scalar-only operators should not appear
        assert!(!ops.iter().any(|o| *o == Operator::Equal));
    }
}
