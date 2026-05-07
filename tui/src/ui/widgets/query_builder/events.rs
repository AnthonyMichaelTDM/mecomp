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

    #[test]
    fn parent_path_of_works() {
        let empty: &[usize] = &[];
        assert_eq!(parent_path_of(empty), empty);
        assert_eq!(parent_path_of(&[0]), empty);
        assert_eq!(parent_path_of(&[0, 1, 2]), &[0, 1]);
    }

    // #[test]
    // fn enter_opens_overlay_for_dropdown_control() {
    //     let mut state = QueryBuilderState::new();
    //     let condition = state.root_conditions().next().expect("condition exists");
    //     let control = ControlRef {
    //         condition_id: condition.id,
    //         kind: LeafFocus::Field,
    //     };

    //     state.set_focused(Some(control));
    //     state.set_control_area(control, Rect::new(1, 1, 10, 1));

    //     let command = handle_key_event(&mut state, KeyEvent::from(KeyCode::Enter));

    //     assert!(matches!(
    //         command,
    //         Some(Action::Overlay(OverlayAction::Open(_)))
    //     ));
    // }

    // #[test]
    // fn space_opens_overlay_for_dropdown_control() {
    //     let mut state = QueryBuilderState::new();
    //     let condition = state.root_conditions().next().expect("condition exists");
    //     state.set_focused(Some(ControlRef {
    //         condition_id: condition.id,
    //         kind: LeafFocus::Operator,
    //     }));

    //     let command = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char(' ')));

    //     assert!(matches!(
    //         command,
    //         Some(Action::Overlay(OverlayAction::Open(_)))
    //     ));
    // }

    // #[test]
    // fn up_down_wrap_row_focus() {
    //     let mut state = QueryBuilderState::new();
    //     let first_id = state.root_conditions().next().expect("condition exists").id;
    //     let second_id = state.add_condition_to_root();

    //     state.set_focused(Some(ControlRef {
    //         condition_id: first_id,
    //         kind: LeafFocus::Field,
    //     }));

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Up));
    //     assert_eq!(
    //         state.focused.map(|focused| focused.condition_id),
    //         Some(second_id)
    //     );

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Down));
    //     assert_eq!(
    //         state.focused.map(|focused| focused.condition_id),
    //         Some(first_id)
    //     );
    // }

    // #[test]
    // fn left_right_cycle_focused_part() {
    //     let mut state = QueryBuilderState::new();
    //     let condition_id = state.root_conditions().next().expect("condition exists").id;
    //     state.set_focused(Some(ControlRef {
    //         condition_id,
    //         kind: LeafFocus::Field,
    //     }));

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Right));
    //     assert_eq!(
    //         state.focused.map(|focused| focused.kind),
    //         Some(LeafFocus::Operator)
    //     );

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Left));
    //     assert_eq!(
    //         state.focused.map(|focused| focused.kind),
    //         Some(LeafFocus::Field)
    //     );
    // }

    // #[test]
    // fn delete_key_removes_focused_condition() {
    //     let mut state = QueryBuilderState::new();
    //     let first_id = state.root_conditions().next().expect("condition exists").id;
    //     let _second_id = state.add_condition_to_root();
    //     state.set_focused(Some(ControlRef {
    //         condition_id: first_id,
    //         kind: LeafFocus::Field,
    //     }));

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Delete));

    //     assert_eq!(state.root_conditions().count(), 1);
    //     assert_ne!(
    //         state.root_conditions().next().expect("condition exists").id,
    //         first_id
    //     );
    // }

    // #[test]
    // fn g_adds_group_node() {
    //     let mut state = QueryBuilderState::new();
    //     let group_count_before = state
    //         .root
    //         .children
    //         .iter()
    //         .filter(|node| matches!(node, super::super::state::QueryNode::Group(_)))
    //         .count();

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char('g')));

    //     let group_count_after = state
    //         .root
    //         .children
    //         .iter()
    //         .filter(|node| matches!(node, super::super::state::QueryNode::Group(_)))
    //         .count();
    //     assert_eq!(group_count_after, group_count_before + 1);
    // }

    // #[test]
    // fn esc_deactivates_before_propagating_close() {
    //     let mut state = QueryBuilderState::new();

    //     let first = handle_key_event(&mut state, KeyEvent::from(KeyCode::Esc));
    //     assert_eq!(first, None);
    //     assert!(state.focused.is_none());

    //     let second = handle_key_event(&mut state, KeyEvent::from(KeyCode::Esc));
    //     assert!(matches!(
    //         second,
    //         Some(Action::Overlay(OverlayAction::Close))
    //     ));
    // }

    // #[test]
    // fn j_no_longer_toggles_join() {
    //     let mut state = QueryBuilderState::new();
    //     let join_before = state.root.join;

    //     let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char('j')));

    //     assert_eq!(state.root.join, join_before);
    // }
}
