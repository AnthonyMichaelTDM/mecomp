use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use ratatui::layout::Rect;

use crate::{
    state::action::{Action, OverlayAction},
    ui::widgets::query_builder::utils::LeafFocus,
};

use super::{
    state::{BuilderMode, QueryBuilderState},
    utils::{FlatNodeKind, flatten_tree},
};

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

    match current_node.kind {
        FlatNodeKind::GroupHeader => handle_group_header_key(state, key, &current_node.path, n),
        FlatNodeKind::Leaf => handle_leaf_key(state, key, &current_node.path, n),
        FlatNodeKind::AddClause => handle_add_clause_key(state, key, &current_node.path, n),
        FlatNodeKind::AddGroup => handle_add_group_key(state, key, &current_node.path, n),
    }
}

fn handle_group_header_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
    flat_len: usize,
) -> Option<Action> {
    match key.code {
        KeyCode::Up => {
            state.cursor.move_up(flat_len);
            None
        }
        KeyCode::Down => {
            state.cursor.move_down(flat_len);
            None
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Open overlay to change group kind
            if let Some(group) = state.group_at_mut(path) {
                Some(Action::Overlay(OverlayAction::Open(
                    group.kind_dd.open_overlay(4),
                )))
            } else {
                None
            }
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

fn handle_leaf_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
    flat_len: usize,
) -> Option<Action> {
    match key.code {
        KeyCode::Up => {
            state.cursor.move_up(flat_len);
            None
        }
        KeyCode::Down => {
            state.cursor.move_down(flat_len);
            None
        }
        KeyCode::Right => {
            if let Some(leaf) = state.leaf_at_mut(path) {
                leaf.leaf_focus = leaf.leaf_focus.next();
            }
            None
        }
        KeyCode::Left => {
            if let Some(leaf) = state.leaf_at_mut(path) {
                leaf.leaf_focus = leaf.leaf_focus.prev();
            }
            None
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Activate the focused sub-element
            if let Some(leaf) = state.leaf_at_mut(path) {
                let overlay = match leaf.leaf_focus {
                    LeafFocus::Field => leaf.field_dd.open_overlay(8),
                    LeafFocus::Operator => leaf.operator_dd.open_overlay(10),
                    LeafFocus::Value => {
                        todo!();
                    }
                };
                Some(Action::Overlay(OverlayAction::Open(overlay)))
            } else {
                None
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            state.remove_child_at(path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.clamp(new_flat.len());
            None
        }
        _ => None,
    }
}

// fn handle_leaf_value_key(
//     state: &mut QueryBuilderState,
//     key: KeyEvent,
//     path: &[usize],
//     flat_len: usize,
// ) -> bool {
//     match key.code {
//         KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
//             // de-focus value; Tab advances leaf focus
//             if let Some(leaf) = state.leaf_at_mut(path) {
//                 match key.code {
//                     KeyCode::Tab => leaf.leaf_focus = leaf.leaf_focus.next(),
//                     KeyCode::BackTab => leaf.leaf_focus = leaf.leaf_focus.prev(),
//                     _ => leaf.leaf_focus = LeafFocus::Field, // Esc resets to Field
//                 }
//             }
//             true
//         }
//         KeyCode::Up => {
//             // Treat as cursor move only if value doesn't intercept it
//             state.cursor.move_up(flat_len);
//             true
//         }
//         KeyCode::Down => {
//             state.cursor.move_down(flat_len);
//             true
//         }
//         _ => {
//             // Delegate to the value input
//             if let Some(leaf) = state.leaf_at_mut(path) {
//                 match &mut leaf.value {
//                     UiValue::Text(input) | UiValue::Integer(input) => {
//                         input.handle_key_event(key);
//                     }
//                     UiValue::Set { items, item_input } => match key.code {
//                         KeyCode::Enter => {
//                             let text = item_input.text().trim().to_string();
//                             if !text.is_empty() {
//                                 items.push(text);
//                                 item_input.clear();
//                             }
//                         }
//                         KeyCode::Backspace if item_input.is_empty() => {
//                             items.pop();
//                         }
//                         _ => item_input.handle_key_event(key),
//                     },
//                 }
//             }
//             true
//         }
//     }
// }

fn handle_add_clause_key(
    state: &mut QueryBuilderState,
    key: KeyEvent,
    path: &[usize],
    flat_len: usize,
) -> Option<Action> {
    match key.code {
        KeyCode::Up => state.cursor.move_up(flat_len),
        KeyCode::Down => state.cursor.move_down(flat_len),
        KeyCode::Enter | KeyCode::Char(' ' | 'a') => {
            // Add clause to the parent group (path with last sentinel removed)
            let parent_path = parent_path_from_add(path);
            state.add_leaf_at(&parent_path);
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
    flat_len: usize,
) -> Option<Action> {
    match key.code {
        KeyCode::Up => state.cursor.move_up(flat_len),
        KeyCode::Down => state.cursor.move_down(flat_len),
        KeyCode::Enter | KeyCode::Char(' ' | 'g') => {
            let parent_path = parent_path_from_add(path);
            state.add_group_at(&parent_path);
            let new_flat = flatten_tree(&state.root);
            state.cursor.flat_index = new_flat.len().saturating_sub(2);
            state.cursor.clamp(new_flat.len());
        }
        _ => {}
    }
    None
}

/// Given the sentinel path of an add-button, return the parent group path.
fn parent_path_from_add(path: &[usize]) -> Vec<usize> {
    if path.len() <= 1 {
        vec![]
    } else {
        path[..path.len() - 1].to_vec()
    }
}

#[must_use]
pub const fn handle_mouse_event(
    _state: &mut QueryBuilderState,
    _mouse: MouseEvent,
    _area: Rect, // area of the entire query builder
) -> Option<Action> {
    None

    // if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
    //     return None;
    // }

    // // find the control that was clicked, if any, and focus it
    // let controls = collect_controls(state);
    // let hit = controls.iter().find_map(|control| {
    //     state
    //         .control_area(*control)
    //         .filter(|area| area.contains((mouse.column, mouse.row).into()))
    //         .map(|_| *control)
    // });

    // // if we hit a control, focus it and open overlay if it's a dropdown control
    // if let Some(control) = hit {
    //     state.set_focused(Some(control));

    //     if matches!(control.kind, LeafFocus::Field | LeafFocus::Operator) {
    //         return state
    //             .open_overlay_for_focused(8)
    //             .map(|overlay| Action::Overlay(OverlayAction::Open(overlay)));
    //     }

    //     return None;
    // }

    // Some(Action::Overlay(OverlayAction::Close))
}

#[cfg(test)]
mod tests {
    // use super::*;

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
