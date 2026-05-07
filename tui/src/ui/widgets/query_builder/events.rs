use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::{
    state::action::{Action, OverlayAction},
    ui::widgets::query_builder::utils::{LeafFocus, UiClause, parent_path_of},
};

use super::{
    state::{BuilderMode, QueryBuilderState},
    utils::{FlatNodeKind, flatten_tree},
};

const KIND_OVERLAY_ROWS: u16 = 2;
const FIELD_OVERLAY_ROWS: u16 = 8;
const OPERATOR_OVERLAY_ROWS: u16 = 10;

#[must_use]
pub fn handle_key_event(state: &mut QueryBuilderState, key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    if state.mode == BuilderMode::RawText {
        state.raw_input.handle_key_event(key);
        state.update_raw_validity();
        return None;
    }

    let flat = flatten_tree(&state.root);
    let n = flat.len();
    if n == 0 {
        return None;
    }

    state.cursor.clamp(n);
    let cursor_idx = state.cursor.flat_index;
    let current_node = flat[cursor_idx].clone();

    // handle global keys first
    match key.code {
        KeyCode::Up => {
            state.cursor.move_up();
            None
        }
        KeyCode::Down => {
            state.cursor.move_down();
            None
        }
        _ => match current_node.kind {
            FlatNodeKind::GroupHeader => handle_group_header_key(state, key, &current_node.path),
            FlatNodeKind::Leaf => handle_leaf_key(state, key, &current_node.path),
            FlatNodeKind::AddClause => handle_add_clause_key(state, key, &current_node.path),
            FlatNodeKind::AddGroup => handle_add_group_key(state, key, &current_node.path),
        },
    }
}

fn handle_group_header_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
) -> Option<Action> {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Open overlay to change group kind
            state
                .clause_at_mut(path)
                .and_then(UiClause::group)
                .map(|group| {
                    Action::Overlay(OverlayAction::Open(
                        group.kind_dd.open_overlay(KIND_OVERLAY_ROWS),
                    ))
                })
        }
        KeyCode::Char('d') | KeyCode::Delete if !path.is_empty() => {
            state.remove_child_at(path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.clamp(new_flat.len());
            None
        }
        _ => None,
    }
}

fn handle_leaf_key(state: &mut QueryBuilderState, key: KeyEvent, path: &[usize]) -> Option<Action> {
    let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(path) else {
        return None;
    };

    match key.code {
        KeyCode::Right => {
            leaf.leaf_focus = leaf.leaf_focus.next();
            None
        }
        KeyCode::Left => {
            leaf.leaf_focus = leaf.leaf_focus.prev();
            None
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Activate the focused sub-element
            let overlay = match leaf.leaf_focus {
                LeafFocus::Field => leaf.field_dd.open_overlay(FIELD_OVERLAY_ROWS),
                LeafFocus::Operator => leaf.operator_dd.open_overlay(OPERATOR_OVERLAY_ROWS),
                LeafFocus::Value => leaf.value.open_overlay(3),
            };
            Some(Action::Overlay(OverlayAction::Open(overlay)))
        }
        KeyCode::Char('d') | KeyCode::Delete if !path.is_empty() => {
            state.remove_child_at(path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.clamp(new_flat.len());
            None
        }
        _ => None,
    }
}

fn handle_add_clause_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
) -> Option<Action> {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Add clause to the parent group (path with last sentinel removed)
            state.add_leaf_at(parent_path_of(path));
            // Move cursor to the newly added leaf
            let new_flat = flatten_tree(&state.root);
            state.cursor.flat_index = new_flat.len().saturating_sub(3); // before add buttons
            state.cursor.clamp(new_flat.len());
        }
        _ => {}
    }
    None
}

fn handle_add_group_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
) -> Option<Action> {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            state.add_group_at(parent_path_of(path));
            let new_flat = flatten_tree(&state.root);
            state.cursor.flat_index = new_flat.len().saturating_sub(2);
            state.cursor.clamp(new_flat.len());
        }
        _ => {}
    }
    None
}

#[must_use]
pub fn handle_mouse_event(
    state: &mut QueryBuilderState,
    mouse: MouseEvent,
    _area: Rect,
) -> Option<Action> {
    // Handle scroll wheel first
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.cursor.move_up();
            return None;
        }
        MouseEventKind::ScrollDown => {
            state.cursor.move_down();
            return None;
        }
        _ => {}
    }

    // Handle left click
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }

    let click_pos = Position::new(mouse.column, mouse.row);

    // Find which clickable region was clicked and clone the data we need
    let region_data = state
        .clickable_regions
        .iter()
        .find(|r| r.area.contains(click_pos))
        .map(|r| (r.action, r.path.clone(), r.flat_index, r.leaf_focus))?;

    let (action, path, flat_index, leaf_focus) = region_data;

    // Move cursor to this element
    state.cursor.flat_index = flat_index;

    // Set leaf focus if applicable
    if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&path)
        && let Some(focus) = leaf_focus
    {
        leaf.leaf_focus = focus;
    }

    // Perform the action
    match action {
        super::state::ClickableAction::GroupKind => {
            // Open the group kind dropdown
            state
                .clause_at_mut(&path)
                .and_then(UiClause::group)
                .map(|group| {
                    Action::Overlay(OverlayAction::Open(
                        group.kind_dd.open_overlay(KIND_OVERLAY_ROWS),
                    ))
                })
        }
        super::state::ClickableAction::LeafField => {
            // Open the field dropdown
            state
                .clause_at_mut(&path)
                .and_then(UiClause::leaf)
                .map(|leaf| {
                    Action::Overlay(OverlayAction::Open(
                        leaf.field_dd.open_overlay(FIELD_OVERLAY_ROWS),
                    ))
                })
        }
        super::state::ClickableAction::LeafOperator => {
            // Open the operator dropdown
            state
                .clause_at_mut(&path)
                .and_then(UiClause::leaf)
                .map(|leaf| {
                    Action::Overlay(OverlayAction::Open(
                        leaf.operator_dd.open_overlay(OPERATOR_OVERLAY_ROWS),
                    ))
                })
        }
        super::state::ClickableAction::LeafValue => {
            // Open the value overlay
            state
                .clause_at_mut(&path)
                .and_then(UiClause::leaf)
                .map(|leaf| Action::Overlay(OverlayAction::Open(leaf.value.open_overlay(3))))
        }
        super::state::ClickableAction::Delete => {
            // Delete the element
            state.remove_child_at(&path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.clamp(new_flat.len());
            None
        }
        super::state::ClickableAction::AddClause => {
            // Add a clause to the parent group
            let parent_path = parent_path_of(&path);
            state.add_leaf_at(parent_path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.flat_index = new_flat.len().saturating_sub(3); // before add buttons
            state.cursor.clamp(new_flat.len());
            None
        }
        super::state::ClickableAction::AddGroup => {
            // Add a group to the parent group
            let parent_path = parent_path_of(&path);
            state.add_group_at(parent_path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.flat_index = new_flat.len().saturating_sub(2);
            state.cursor.clamp(new_flat.len());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::query_builder::state::{ClickableAction, ClickableRegion};
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
    use ratatui::layout::Position;

    #[test]
    fn parent_path_of_works() {
        let empty: &[usize] = &[];
        assert_eq!(parent_path_of(empty), empty);
        assert_eq!(parent_path_of(&[0]), empty);
        assert_eq!(parent_path_of(&[0, 1, 2]), &[0, 1]);
        assert_eq!(parent_path_of(&[0, 1, 2, 3, 4, 5]), &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn key_event_ignores_releases() {
        let mut state = QueryBuilderState::default();
        let key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Release,
            state: crossterm::event::KeyEventState::empty(),
        };
        let result = handle_key_event(&mut state, key);
        assert_eq!(result, None);
    }

    #[test]
    fn key_event_ignores_repeats() {
        let mut state = QueryBuilderState::default();
        let key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Repeat,
            state: crossterm::event::KeyEventState::empty(),
        };
        let result = handle_key_event(&mut state, key);
        assert_eq!(result, None);
    }

    #[test]
    fn up_key_moves_cursor_up() {
        let mut state = QueryBuilderState::default();
        // Default flat list has 3 nodes: 1 leaf + 2 add buttons
        // Cursor should start at 0
        assert_eq!(state.cursor.flat_index, 0);

        state.cursor.flat_index = 2;
        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, 1);
    }

    #[test]
    fn down_key_moves_cursor_down() {
        let mut state = QueryBuilderState::default();
        assert_eq!(state.cursor.flat_index, 0);

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, 1);
    }

    #[test]
    fn up_key_wraps_around_at_top() {
        let mut state = QueryBuilderState::default();
        state.cursor.flat_index = 0;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        // Should wrap to the last item (index 2 in default 3-item list)
        assert_eq!(state.cursor.flat_index, 2);
    }

    #[test]
    fn down_key_wraps_around_at_bottom() {
        let mut state = QueryBuilderState::default();
        state.cursor.flat_index = 2;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        // Should wrap to the first item (index 0)
        assert_eq!(state.cursor.flat_index, 0);
    }

    #[test]
    fn leaf_right_key_advances_leaf_focus() {
        let mut state = QueryBuilderState::default();
        // The cursor should be on the leaf
        let flat = flatten_tree(&state.root);
        assert_eq!(flat[0].kind, FlatNodeKind::Leaf);

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&[]) {
            assert_eq!(leaf.leaf_focus, LeafFocus::Operator);
        } else {
            panic!("Expected leaf at root");
        }
    }

    #[test]
    fn leaf_left_key_moves_back_in_leaf_focus() {
        let mut state = QueryBuilderState::default();
        if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&[]) {
            leaf.leaf_focus = LeafFocus::Operator;
        }

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&[]) {
            assert_eq!(leaf.leaf_focus, LeafFocus::Field);
        } else {
            panic!("Expected leaf at root");
        }
    }

    #[test]
    fn leaf_focus_cycles_through_field_operator_value() {
        if let Some(UiClause::Leaf(_leaf)) = {
            let mut state = QueryBuilderState::default();
            state.clause_at_mut(&[]).cloned()
        } {
            assert_eq!(LeafFocus::Field.next(), LeafFocus::Operator);
            assert_eq!(LeafFocus::Operator.next(), LeafFocus::Value);
            assert_eq!(LeafFocus::Value.next(), LeafFocus::Field);

            assert_eq!(LeafFocus::Field.prev(), LeafFocus::Value);
            assert_eq!(LeafFocus::Value.prev(), LeafFocus::Operator);
            assert_eq!(LeafFocus::Operator.prev(), LeafFocus::Field);
        }
    }

    #[test]
    fn leaf_enter_opens_field_overlay() {
        let mut state = QueryBuilderState::default();
        if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&[]) {
            leaf.leaf_focus = LeafFocus::Field;
        }

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert!(matches!(
            result,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn leaf_space_opens_operator_overlay() {
        let mut state = QueryBuilderState::default();
        if let Some(UiClause::Leaf(leaf)) = state.clause_at_mut(&[]) {
            leaf.leaf_focus = LeafFocus::Operator;
        }

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert!(matches!(
            result,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn leaf_delete_removes_leaf_when_not_root() {
        let mut state = QueryBuilderState::default();
        // Add leaves to create a group: first converts leaf to group, second adds another leaf
        state.add_leaf_at(&[]);
        state.add_leaf_at(&[]);

        // Now we have a group with 3 leaves. Delete one and we should still have a group with 2.
        let initial_count = if let Some(UiClause::Group(g)) = state.clause_at_mut(&[]) {
            g.clauses.len()
        } else {
            panic!("Root should be a group after adding leaves");
        };
        assert_eq!(initial_count, 3);

        // Move cursor to the last leaf (index 3: header=0, leaf1=1, leaf2=2, leaf3=3)
        state.cursor.flat_index = 3;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        if let Some(UiClause::Group(g)) = state.clause_at_mut(&[]) {
            assert_eq!(g.clauses.len(), initial_count - 1);
        } else {
            panic!("Expected root to still be a group after deletion");
        }
    }

    #[test]
    fn group_header_enter_opens_kind_overlay() {
        let mut state = QueryBuilderState::default();
        // Convert to group
        state.add_leaf_at(&[]);

        // Move cursor to group header (index 0)
        state.cursor.flat_index = 0;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert!(matches!(
            result,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn add_clause_button_creates_new_leaf() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);
        // Find the AddClause button
        let flat = flatten_tree(&state.root);
        let add_clause_idx = flat
            .iter()
            .position(|n| n.kind == FlatNodeKind::AddClause)
            .expect("AddClause button should exist");

        state.cursor.flat_index = add_clause_idx;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        // Verify a new leaf was added
        let new_flat = flatten_tree(&state.root);
        let leaf_count = new_flat
            .iter()
            .filter(|n| n.kind == FlatNodeKind::Leaf)
            .count();
        assert_eq!(leaf_count, 3); // 2 original + 1 new
    }

    #[test]
    fn add_group_button_creates_new_group() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);
        let flat = flatten_tree(&state.root);
        let add_group_idx = flat
            .iter()
            .position(|n| n.kind == FlatNodeKind::AddGroup)
            .expect("AddGroup button should exist");

        state.cursor.flat_index = add_group_idx;

        let result = handle_key_event(
            &mut state,
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
        );

        assert_eq!(result, None);
        // Verify structure changed (new group added)
        let new_flat = flatten_tree(&state.root);
        assert!(new_flat.iter().any(|n| n.kind == FlatNodeKind::GroupHeader));
    }

    #[test]
    fn mouse_scroll_up_moves_cursor() {
        let mut state = QueryBuilderState::default();
        state.cursor.flat_index = 2;

        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, 1);
    }

    #[test]
    fn mouse_scroll_down_moves_cursor() {
        let mut state = QueryBuilderState::default();
        state.cursor.flat_index = 0;

        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, 1);
    }

    #[test]
    fn mouse_ignores_non_left_clicks() {
        let mut state = QueryBuilderState::default();
        let initial_index = state.cursor.flat_index;

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, initial_index);
    }

    #[test]
    fn mouse_ignores_click_outside_regions() {
        let mut state = QueryBuilderState::default();
        let initial_index = state.cursor.flat_index;

        // Click at position (100, 100) which is far outside our 80x20 area
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 100,
            row: 100,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        assert_eq!(state.cursor.flat_index, initial_index);
    }

    #[test]
    fn mouse_left_click_on_region_moves_cursor() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        // Add a clickable region for testing
        let region = ClickableRegion {
            area: Rect::new(10, 5, 20, 1),
            action: ClickableAction::LeafField,
            path: vec![0],
            flat_index: 1,
            leaf_focus: Some(LeafFocus::Field),
        };
        state.clickable_regions.push(region);

        // Click within the region
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert!(matches!(
            result,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
        assert_eq!(state.cursor.flat_index, 1);
    }

    #[test]
    fn mouse_click_sets_leaf_focus() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        let region = ClickableRegion {
            area: Rect::new(10, 5, 20, 1),
            action: ClickableAction::LeafOperator,
            path: vec![0],
            flat_index: 1,
            leaf_focus: Some(LeafFocus::Operator),
        };
        state.clickable_regions.push(region);

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert!(result.is_some());
        if let Some(UiClause::Group(g)) = state.clause_at_mut(&[]) {
            if let UiClause::Leaf(leaf) = &g.clauses[0] {
                assert_eq!(leaf.leaf_focus, LeafFocus::Operator);
            }
        }
    }

    #[test]
    fn mouse_delete_action_removes_element() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        let region = ClickableRegion {
            area: Rect::new(10, 5, 20, 1),
            action: ClickableAction::Delete,
            path: vec![1],
            flat_index: 2,
            leaf_focus: None,
        };
        state.clickable_regions.push(region);

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        if let Some(UiClause::Group(g)) = state.clause_at_mut(&[]) {
            // Should have one less clause
            assert!(g.clauses.len() < 3);
        }
    }

    #[test]
    fn mouse_add_clause_action_creates_leaf() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        let region = ClickableRegion {
            area: Rect::new(10, 5, 20, 1),
            action: ClickableAction::AddClause,
            path: vec![usize::MAX - 1], // sentinel for add button
            flat_index: 3,
            leaf_focus: None,
        };
        state.clickable_regions.push(region);

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        // Verify a new leaf was added
        let new_flat = flatten_tree(&state.root);
        let leaf_count = new_flat
            .iter()
            .filter(|n| n.kind == FlatNodeKind::Leaf)
            .count();
        assert!(leaf_count >= 3);
    }

    #[test]
    fn mouse_add_group_action_creates_group() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        let region = ClickableRegion {
            area: Rect::new(10, 6, 20, 1),
            action: ClickableAction::AddGroup,
            path: vec![usize::MAX],
            flat_index: 4,
            leaf_focus: None,
        };
        state.clickable_regions.push(region);

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 6,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert_eq!(result, None);
        // Verify structure changed
        let new_flat = flatten_tree(&state.root);
        assert!(new_flat.iter().any(|n| n.kind == FlatNodeKind::GroupHeader));
    }

    #[test]
    fn mouse_group_kind_action_opens_overlay() {
        let mut state = QueryBuilderState::default();
        state.add_leaf_at(&[]);

        let region = ClickableRegion {
            area: Rect::new(10, 0, 20, 1),
            action: ClickableAction::GroupKind,
            path: vec![],
            flat_index: 0,
            leaf_focus: None,
        };
        state.clickable_regions.push(region);

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let area = Rect::new(0, 0, 80, 20);

        let result = handle_mouse_event(&mut state, mouse, area);

        assert!(matches!(
            result,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn position_detection_works_at_boundaries() {
        let rect = Rect::new(10, 5, 20, 3); // x: 10-29, y: 5-7

        let pos_inside_top_left = Position::new(10, 5);
        let pos_inside_bottom_right = Position::new(29, 7);
        let pos_outside_left = Position::new(9, 5);
        let pos_outside_right = Position::new(30, 5);
        let pos_outside_top = Position::new(10, 4);
        let pos_outside_bottom = Position::new(10, 8);

        assert!(rect.contains(pos_inside_top_left));
        assert!(rect.contains(pos_inside_bottom_right));
        assert!(!rect.contains(pos_outside_left));
        assert!(!rect.contains(pos_outside_right));
        assert!(!rect.contains(pos_outside_top));
        assert!(!rect.contains(pos_outside_bottom));
    }
}
