use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::state::action::{Action, OverlayAction};

use super::state::{BuilderMode, ControlKind, ControlRef, QueryBuilderState};

#[must_use]
pub fn handle_key_event(state: &mut QueryBuilderState, key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    if key.code == KeyCode::Char('r') {
        state.toggle_mode();
        return None;
    }

    if state.mode == BuilderMode::RawText {
        state.raw_input.handle_key_event(key);
        state.update_raw_validity();
        return None;
    }

    match key.code {
        KeyCode::Up => {
            state.move_focus_up();
            None
        }
        KeyCode::Down => {
            state.move_focus_down();
            None
        }
        KeyCode::Left => {
            state.cycle_focused_part(true);
            None
        }
        KeyCode::Right => {
            state.cycle_focused_part(false);
            None
        }
        KeyCode::Char('a') => {
            state.add_condition_to_root();
            None
        }
        KeyCode::Char('g') => {
            state.add_group_to_root();
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            state.delete_focused_condition();
            None
        }
        KeyCode::Enter | KeyCode::Char(' ') => state
            .open_overlay_for_focused(8)
            .map(|overlay| Action::Overlay(OverlayAction::Open(overlay))),
        KeyCode::Esc => {
            if state.deactivate_focused() {
                None
            } else {
                Some(Action::Overlay(OverlayAction::Close))
            }
        }
        KeyCode::Backspace | KeyCode::Char(_) => {
            state.edit_value_text(key);
            None
        }
        _ => None,
    }
}

#[must_use]
pub fn handle_mouse_event(
    state: &mut QueryBuilderState,
    mouse: MouseEvent,
    _area: Rect, // area of the entire query builder
) -> Option<Action> {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }

    // find the control that was clicked, if any, and focus it
    let controls = collect_controls(state);
    let hit = controls.iter().find_map(|control| {
        state
            .control_area(*control)
            .filter(|area| area.contains((mouse.column, mouse.row).into()))
            .map(|_| *control)
    });

    // if we hit a control, focus it and open overlay if it's a dropdown control
    if let Some(control) = hit {
        state.set_focused(Some(control));

        if matches!(control.kind, ControlKind::Field | ControlKind::Operator) {
            return state
                .open_overlay_for_focused(8)
                .map(|overlay| Action::Overlay(OverlayAction::Open(overlay)));
        }

        return None;
    }

    Some(Action::Overlay(OverlayAction::Close))
}

fn collect_controls(state: &QueryBuilderState) -> Vec<ControlRef> {
    state
        .root_conditions()
        .flat_map(|condition| {
            [
                ControlRef {
                    condition_id: condition.id,
                    kind: ControlKind::Field,
                },
                ControlRef {
                    condition_id: condition.id,
                    kind: ControlKind::Operator,
                },
                ControlRef {
                    condition_id: condition.id,
                    kind: ControlKind::Value,
                },
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_opens_overlay_for_dropdown_control() {
        let mut state = QueryBuilderState::new();
        let condition = state.root_conditions().next().expect("condition exists");
        let control = ControlRef {
            condition_id: condition.id,
            kind: ControlKind::Field,
        };

        state.set_focused(Some(control));
        state.set_control_area(control, Rect::new(1, 1, 10, 1));

        let command = handle_key_event(&mut state, KeyEvent::from(KeyCode::Enter));

        assert!(matches!(
            command,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn space_opens_overlay_for_dropdown_control() {
        let mut state = QueryBuilderState::new();
        let condition = state.root_conditions().next().expect("condition exists");
        state.set_focused(Some(ControlRef {
            condition_id: condition.id,
            kind: ControlKind::Operator,
        }));

        let command = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char(' ')));

        assert!(matches!(
            command,
            Some(Action::Overlay(OverlayAction::Open(_)))
        ));
    }

    #[test]
    fn up_down_wrap_row_focus() {
        let mut state = QueryBuilderState::new();
        let first_id = state.root_conditions().next().expect("condition exists").id;
        let second_id = state.add_condition_to_root();

        state.set_focused(Some(ControlRef {
            condition_id: first_id,
            kind: ControlKind::Field,
        }));

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Up));
        assert_eq!(state.focused.map(|focused| focused.condition_id), Some(second_id));

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Down));
        assert_eq!(state.focused.map(|focused| focused.condition_id), Some(first_id));
    }

    #[test]
    fn left_right_cycle_focused_part() {
        let mut state = QueryBuilderState::new();
        let condition_id = state.root_conditions().next().expect("condition exists").id;
        state.set_focused(Some(ControlRef {
            condition_id,
            kind: ControlKind::Field,
        }));

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Right));
        assert_eq!(state.focused.map(|focused| focused.kind), Some(ControlKind::Operator));

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Left));
        assert_eq!(state.focused.map(|focused| focused.kind), Some(ControlKind::Field));
    }

    #[test]
    fn delete_key_removes_focused_condition() {
        let mut state = QueryBuilderState::new();
        let first_id = state.root_conditions().next().expect("condition exists").id;
        let _second_id = state.add_condition_to_root();
        state.set_focused(Some(ControlRef {
            condition_id: first_id,
            kind: ControlKind::Field,
        }));

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Delete));

        assert_eq!(state.root_conditions().count(), 1);
        assert_ne!(state.root_conditions().next().expect("condition exists").id, first_id);
    }

    #[test]
    fn g_adds_group_node() {
        let mut state = QueryBuilderState::new();
        let group_count_before = state
            .root
            .children
            .iter()
            .filter(|node| matches!(node, super::super::state::QueryNode::Group(_)))
            .count();

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char('g')));

        let group_count_after = state
            .root
            .children
            .iter()
            .filter(|node| matches!(node, super::super::state::QueryNode::Group(_)))
            .count();
        assert_eq!(group_count_after, group_count_before + 1);
    }

    #[test]
    fn esc_deactivates_before_propagating_close() {
        let mut state = QueryBuilderState::new();

        let first = handle_key_event(&mut state, KeyEvent::from(KeyCode::Esc));
        assert_eq!(first, None);
        assert!(state.focused.is_none());

        let second = handle_key_event(&mut state, KeyEvent::from(KeyCode::Esc));
        assert!(matches!(second, Some(Action::Overlay(OverlayAction::Close))));
    }

    #[test]
    fn j_no_longer_toggles_join() {
        let mut state = QueryBuilderState::new();
        let join_before = state.root.join;

        let _ = handle_key_event(&mut state, KeyEvent::from(KeyCode::Char('j')));

        assert_eq!(state.root.join, join_before);
    }
}
