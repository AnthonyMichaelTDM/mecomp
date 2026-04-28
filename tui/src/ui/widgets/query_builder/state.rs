//! UI-side data model for the query builder.

use std::{collections::HashMap, str::FromStr};

use mecomp_storage::db::schemas::dynamic::query::{
    Clause, Compile as _, CompoundClause, CompoundKind, Field, LeafClause, Operator, Query, Value,
};
use ratatui::layout::Rect;
use strum::IntoEnumIterator;

use crate::ui::widgets::{
    dropdown::DropdownState,
    input_box::InputBoxState,
    overlay::{OverlayResult, OverlayType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuilderMode {
    #[default]
    Visual,
    RawText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlKind {
    Field,
    Operator,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlRef {
    pub condition_id: u64,
    pub kind: ControlKind,
}

#[derive(Debug, Clone)]
pub struct QueryCondition {
    pub id: u64,
    pub field: DropdownState<Field>,
    pub operator: DropdownState<Operator>,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueryGroup {
    pub id: u64,
    pub join: CompoundKind,
    pub children: Vec<QueryNode>,
}

#[derive(Debug, Clone)]
pub enum QueryNode {
    Group(QueryGroup),
    Condition(QueryCondition),
}

#[derive(Debug, Clone)]
pub struct QueryBuilderState {
    pub mode: BuilderMode,
    pub root: QueryGroup,
    pub focused: Option<ControlRef>,
    pub raw_input: InputBoxState,
    pub raw_input_valid: bool,
    control_areas: HashMap<ControlRef, Rect>,
    next_id: u64,
}

impl Default for QueryBuilderState {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryBuilderState {
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self {
            mode: BuilderMode::Visual,
            root: QueryGroup {
                id: 1,
                join: CompoundKind::And,
                children: Vec::new(),
            },
            focused: None,
            raw_input: InputBoxState::new(),
            raw_input_valid: false,
            control_areas: HashMap::new(),
            next_id: 2,
        };

        let condition = state.new_condition();
        state.focused = Some(ControlRef {
            condition_id: condition.id,
            kind: ControlKind::Field,
        });
        state.root.children.push(QueryNode::Condition(condition));
        if let Some(query) = state.try_to_query() {
            state.raw_input.set_text(&query.compile_for_storage());
            state.raw_input_valid = true;
        }

        state
    }

    fn new_condition(&mut self) -> QueryCondition {
        let id = self.alloc_id();
        let field_options = Field::iter();
        let operator_options = Operator::iter();

        QueryCondition {
            id,
            field: DropdownState::new(Self::control_id(id, ControlKind::Field), field_options),
            operator: DropdownState::new(
                Self::control_id(id, ControlKind::Operator),
                operator_options,
            ),
            value: String::new(),
        }
    }

    const fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    #[must_use]
    pub const fn control_id(condition_id: u64, kind: ControlKind) -> u64 {
        let slot = match kind {
            ControlKind::Field => 1,
            ControlKind::Operator => 2,
            ControlKind::Value => 3,
        };
        condition_id * 10 + slot
    }

    pub fn set_control_area(&mut self, control: ControlRef, area: Rect) {
        self.control_areas.insert(control, area);
    }

    #[must_use]
    pub fn control_area(&self, control: ControlRef) -> Option<Rect> {
        self.control_areas.get(&control).copied()
    }

    pub fn clear_control_areas(&mut self) {
        self.control_areas.clear();
    }

    pub fn add_condition_to_root(&mut self) -> u64 {
        let condition = self.new_condition();
        let id = condition.id;
        self.root.children.push(QueryNode::Condition(condition));
        self.focused = Some(ControlRef {
            condition_id: id,
            kind: ControlKind::Field,
        });
        self.sync_raw_from_visual();
        id
    }

    pub fn toggle_root_join(&mut self) {
        self.root.join = match self.root.join {
            CompoundKind::And => CompoundKind::Or,
            CompoundKind::Or => CompoundKind::And,
        };
        self.sync_raw_from_visual();
    }

    #[must_use]
    pub fn open_overlay_for_focused(&mut self, max_rows: u16) -> Option<OverlayType> {
        let focused = self.focused?;
        let condition = self.find_condition_mut(focused.condition_id)?;

        match focused.kind {
            ControlKind::Field => Some(condition.field.open_overlay(max_rows)),
            ControlKind::Operator => Some(condition.operator.open_overlay(max_rows)),
            ControlKind::Value => None,
        }
    }

    pub fn apply_overlay_result(&mut self, result: &OverlayResult) -> bool {
        let changed = self
            .iter_conditions_mut()
            .any(|condition| condition.field.apply_overlay_result(result))
            || self
                .iter_conditions_mut()
                .any(|condition| condition.operator.apply_overlay_result(result));

        if changed {
            self.sync_raw_from_visual();
        }

        changed
    }

    pub const fn set_focused(&mut self, focused: Option<ControlRef>) {
        self.focused = focused;
    }

    pub fn root_conditions(&self) -> impl Iterator<Item = &QueryCondition> {
        self.root.children.iter().filter_map(|node| match node {
            QueryNode::Condition(condition) => Some(condition),
            QueryNode::Group(_) => None,
        })
    }

    pub fn root_conditions_mut(&mut self) -> impl Iterator<Item = &mut QueryCondition> {
        self.root.children.iter_mut().filter_map(|node| match node {
            QueryNode::Condition(condition) => Some(condition),
            QueryNode::Group(_) => None,
        })
    }

    fn find_condition_mut(&mut self, id: u64) -> Option<&mut QueryCondition> {
        self.iter_conditions_mut()
            .find(|condition| condition.id == id)
    }

    fn iter_conditions_mut(&mut self) -> impl Iterator<Item = &mut QueryCondition> {
        self.root.children.iter_mut().filter_map(|node| match node {
            QueryNode::Condition(condition) => Some(condition),
            QueryNode::Group(_) => None,
        })
    }

    #[must_use]
    pub fn try_to_query(&self) -> Option<Query> {
        match self.mode {
            BuilderMode::RawText => Query::from_str(self.raw_input.text()).ok(),
            BuilderMode::Visual => self.visual_to_query(),
        }
    }

    pub fn load_query(&mut self, query: &Query) {
        self.root.children.clear();

        match &query.root {
            Clause::Leaf(leaf) => {
                if let Some(condition) = self.condition_from_leaf(leaf) {
                    self.root.children.push(QueryNode::Condition(condition));
                    self.root.join = CompoundKind::And;
                }
            }
            Clause::Compound(compound) => {
                self.root.join = compound.kind;
                for clause in &compound.clauses {
                    if let Clause::Leaf(leaf) = clause
                        && let Some(condition) = self.condition_from_leaf(leaf)
                    {
                        self.root.children.push(QueryNode::Condition(condition));
                    }
                }
            }
        }

        if self.root.children.is_empty() {
            let condition = self.new_condition();
            self.root.children.push(QueryNode::Condition(condition));
        }

        let focus_id = self
            .root_conditions()
            .next()
            .map_or(1, |condition| condition.id);
        self.focused = Some(ControlRef {
            condition_id: focus_id,
            kind: ControlKind::Field,
        });
        self.raw_input.set_text(&query.compile_for_storage());
        self.raw_input_valid = true;
        self.mode = BuilderMode::Visual;
    }

    pub fn toggle_mode(&mut self) {
        match self.mode {
            BuilderMode::Visual => {
                self.sync_raw_from_visual();
                self.mode = BuilderMode::RawText;
            }
            BuilderMode::RawText => {
                if let Ok(query) = Query::from_str(self.raw_input.text()) {
                    self.load_query(&query);
                }
                self.mode = BuilderMode::Visual;
            }
        }
    }

    pub fn update_raw_validity(&mut self) {
        self.raw_input_valid = Query::from_str(self.raw_input.text()).is_ok();
    }

    pub fn edit_value_text(&mut self, key: crossterm::event::KeyEvent) {
        if let Some(focused) = self.focused
            && focused.kind == ControlKind::Value
            && let Some(condition) = self.find_condition_mut(focused.condition_id)
        {
            use crossterm::event::KeyCode;
            match key.code {
                KeyCode::Char(c) => condition.value.push(c),
                KeyCode::Backspace => {
                    condition.value.pop();
                }
                _ => {}
            }
            self.sync_raw_from_visual();
        }
    }

    fn sync_raw_from_visual(&mut self) {
        if let Some(query) = self.visual_to_query() {
            self.raw_input.set_text(&query.compile_for_storage());
            self.raw_input_valid = true;
        } else {
            self.raw_input.set_text("");
            self.raw_input_valid = false;
        }
    }

    fn visual_to_query(&self) -> Option<Query> {
        let leaves = self
            .root_conditions()
            .filter_map(|condition| condition_to_leaf_clause(condition).map(Clause::Leaf))
            .collect::<Vec<_>>();

        match leaves.len() {
            0 => None,
            1 => Some(Query {
                root: leaves.into_iter().next()?,
            }),
            _ => Some(Query {
                root: Clause::Compound(CompoundClause {
                    kind: self.root.join,
                    clauses: leaves,
                }),
            }),
        }
    }

    fn condition_from_leaf(&mut self, leaf: &LeafClause) -> Option<QueryCondition> {
        let Value::Field(field) = &leaf.left else {
            return None;
        };

        let mut condition = self.new_condition();
        let field_text = field.compile_for_storage();
        let op_text = leaf.operator.compile_for_storage();

        let _ = condition.field.select_by_text(&field_text);
        let _ = condition.operator.select_by_text(&op_text);

        condition.value = match &leaf.right {
            Value::String(text) => text.clone(),
            Value::Int(v) => v.to_string(),
            Value::Field(f) => f.compile_for_storage(),
            Value::Set(items) => items
                .iter()
                .map(|value| match value {
                    Value::String(text) => text.clone(),
                    Value::Int(v) => v.to_string(),
                    Value::Field(f) => f.compile_for_storage(),
                    Value::Set(_) => String::new(),
                })
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(","),
        };

        Some(condition)
    }
}

fn condition_to_leaf_clause(condition: &QueryCondition) -> Option<LeafClause> {
    let field = Field::from_str(condition.field.selected()?).ok()?;
    let operator = Operator::from_str(condition.operator.selected()?).ok()?;
    let right = if field == Field::ReleaseYear {
        Value::Int(condition.value.parse().ok()?)
    } else {
        Value::String(condition.value.clone())
    };

    let leaf = LeafClause {
        left: Value::Field(field),
        operator,
        right,
    };

    if leaf.has_valid_operator() {
        Some(leaf)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mecomp_storage::db::schemas::dynamic::query::Query;

    #[test]
    fn open_overlay_for_focused_uses_control_id() {
        let mut state = QueryBuilderState::new();
        let condition_id = state.root_conditions().next().expect("condition exists").id;
        let focused = ControlRef {
            condition_id,
            kind: ControlKind::Field,
        };
        state.set_focused(Some(focused));
        state.set_control_area(focused, Rect::new(2, 2, 12, 1));

        let overlay = state.open_overlay_for_focused(6).expect("overlay expected");

        let crate::ui::widgets::overlay::OverlayType::Dropdown(dropdown) = overlay;
        assert_eq!(
            dropdown.target_id,
            QueryBuilderState::control_id(condition_id, ControlKind::Field)
        );
    }

    #[test]
    fn apply_overlay_result_updates_matching_dropdown() {
        let mut state = QueryBuilderState::new();
        let condition_id = state.root_conditions().next().expect("condition exists").id;
        let target_id = QueryBuilderState::control_id(condition_id, ControlKind::Operator);

        let changed = state.apply_overlay_result(&OverlayResult::DropdownSelected {
            target_id,
            selected_index: 1,
        });

        assert!(changed);
    }

    #[test]
    fn toggle_mode_round_trip() {
        let mut state = QueryBuilderState::new();
        state.toggle_mode();
        assert_eq!(state.mode, BuilderMode::RawText);
        state.toggle_mode();
        assert_eq!(state.mode, BuilderMode::Visual);
    }

    #[test]
    fn query_compile_in_visual_mode() {
        let state = QueryBuilderState::new();
        let query = state.try_to_query().expect("query exists");
        assert!(!query.compile_for_storage().is_empty());
    }

    #[test]
    fn raw_mode_parse_works() {
        let mut state = QueryBuilderState::new();
        state.mode = BuilderMode::RawText;
        state.raw_input.set_text("title = \"foo\"");
        let query = state.try_to_query().expect("query parsed");
        assert_eq!(query.compile_for_storage(), "title = \"foo\"");
    }

    #[test]
    fn load_query_sets_visual_state() {
        let mut state = QueryBuilderState::new();
        let query = Query::from_str("title = \"abc\"").expect("query parse");
        state.load_query(&query);
        assert_eq!(state.mode, BuilderMode::Visual);
        assert!(state.root_conditions().next().is_some());
    }
}
