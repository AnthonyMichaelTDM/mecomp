use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Offset, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::ui::{
    colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL},
    widgets::{dropdown::Dropdown, input_box::InputBox},
};

use super::{
    state::{BuilderMode, ClickableAction, QueryBuilderState},
    utils::{FlatNode, FlatNodeKind, LeafFocus, UiValue, flatten_tree},
};

pub fn render_query_builder(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    area: Rect,
    is_focused: bool,
) {
    match state.mode {
        BuilderMode::Visual => render_visual_mode(frame, state, area, is_focused),
        BuilderMode::RawText => render_raw_mode(frame, state, area, is_focused),
    }
}

fn render_raw_mode(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    area: Rect,
    is_focused: bool,
) {
    let status = if state.raw_input_valid {
        "Enter Query"
    } else {
        "Invalid Query"
    };

    let border_color: ratatui::style::Color = if state.raw_input_valid {
        if is_focused {
            *BORDER_FOCUSED
        } else {
            *BORDER_UNFOCUSED
        }
    } else {
        *TEXT_HIGHLIGHT
    }
    .into();

    let border = Block::bordered()
        .title_top(Line::from(vec![
            Span::styled(
                "Query Builder",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "Raw(Shift+Tab: visual mode)",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]))
        .title_bottom(status)
        .border_style(Style::default().fg(border_color));

    let text_color = if is_focused {
        (*TEXT_NORMAL).into()
    } else {
        (*TEXT_HIGHLIGHT_ALT).into()
    };

    let query_input = InputBox::new().border(border).text_color(text_color);
    frame.render_stateful_widget(query_input, area, &mut state.raw_input);

    // update cursor position
    if is_focused {
        let position = area + state.raw_input.cursor_offset() + Offset::new(1, 1);
        frame.set_cursor_position(position);
    }
}

fn render_visual_mode(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    area: Rect,
    is_focused: bool,
) {
    // Clear clickable regions before rendering
    state.clickable_regions.clear();

    let focused_color: ratatui::style::Color = if is_focused {
        (*BORDER_FOCUSED).into()
    } else {
        (*BORDER_UNFOCUSED).into()
    };

    let border = Block::bordered()
        .title_top(Line::from(vec![
            Span::styled(
                "Query Builder",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "Visual(Tab: raw mode)",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]))
        .border_style(Style::default().fg(focused_color))
        .title_bottom("↑/↓/←/→: focus | Enter: open | a: add | g: group | d/Del: remove");

    let inner = border.inner(area);
    frame.render_widget(border, area);

    // Build the flat list to know how many items there are and where the cursor is.
    let flat = flatten_tree(&state.root);
    let n = flat.len();
    state.cursor.clamp(n);
    let cursor_idx = state.cursor.flat_index;

    // We render one row per flat item.
    let visible_rows = inner.height as usize;

    // Determine scroll offset: prefer the user's manual scroll, but ensure cursor is in view
    let scroll_offset = state
        .scroll_offset
        // If cursor is above the visible area, scroll up to show it
        .min(cursor_idx)
        // If cursor is below the visible area, scroll down to show it
        .max(cursor_idx.saturating_sub(visible_rows - 1))
        // Ensure we don't scroll past the end
        .min(n.saturating_sub(visible_rows));

    // Update the stored scroll offset
    state.scroll_offset = scroll_offset;

    // render rows
    for ((rel_i, node), row_num) in flat.iter().enumerate().skip(scroll_offset).zip(0..) {
        let row_area = inner.offset(Offset::new(0, row_num)).intersection(inner);
        if row_area.height == 0 {
            break;
        }
        let is_cursor = rel_i == cursor_idx;
        render_flat_node(frame, state, node, row_area, is_cursor, rel_i);
    }
}

fn render_flat_node(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    node: &FlatNode,
    area: Rect,
    is_cursor: bool,
    flat_index: usize,
) {
    #[allow(clippy::cast_possible_truncation)]
    let indent = (node.depth * 2) as u16;
    let [_, inner_area] = area.layout(&Layout::horizontal([
        Constraint::Length(indent),
        Constraint::Min(0),
    ]));

    if inner_area.width == 0 {
        return;
    }

    let cursor_style = if is_cursor {
        Style::default().bg((*BORDER_UNFOCUSED).into())
    } else {
        Style::default()
    };

    match node.kind {
        FlatNodeKind::GroupHeader => {
            render_group_header(
                frame,
                state,
                &node.path,
                inner_area,
                cursor_style,
                flat_index,
            );
        }
        FlatNodeKind::Leaf => {
            render_leaf_row(
                frame,
                state,
                &node.path,
                inner_area,
                cursor_style,
                flat_index,
            );
        }
        FlatNodeKind::AddClause => {
            let label = Span::styled("[+ Add Clause]", cursor_style);
            frame.render_widget(Paragraph::new(Line::from(label)), inner_area);
            state
                .clickable_regions
                .push(ClickableAction::AddClause.region(inner_area, node.path.clone(), flat_index));
        }
        FlatNodeKind::AddGroup => {
            let label = Span::styled("[+ Add Group]", cursor_style);
            frame.render_widget(Paragraph::new(Line::from(label)), inner_area);
            state
                .clickable_regions
                .push(ClickableAction::AddGroup.region(inner_area, node.path.clone(), flat_index));
        }
    }
}

fn render_group_header(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    path: &[usize],
    area: Rect,
    row_style: Style,
    flat_index: usize,
) {
    let Some(group) = state.group_at_mut(path) else {
        return;
    };

    // Allocate space: "[kind ▼]  [-Del]"
    // kind_dd takes 8 cols, del button 6 cols
    let [kind_area, _, del_area] = area.layout(&Layout::horizontal([
        Constraint::Length(8),
        Constraint::Min(0),
        Constraint::Length(6),
    ]));

    let kind_dd = Dropdown::new().style(row_style);
    frame.render_stateful_widget(kind_dd, area, &mut group.kind_dd);
    // Record the clickable region for the kind dropdown
    state
        .clickable_regions
        .push(ClickableAction::GroupKind.region(kind_area, path.to_vec(), flat_index));

    // Del button if non-root
    if !path.is_empty() {
        let del_style = Style::default().fg((*TEXT_HIGHLIGHT).into());
        frame.render_widget(Line::from("[-Del]").style(del_style), del_area);
        // Record the clickable region for the delete button
        state.clickable_regions.push(ClickableAction::Delete.region(
            del_area,
            path.to_vec(),
            flat_index,
        ));
    }
}

fn render_leaf_row(
    frame: &mut Frame<'_>,
    state: &mut QueryBuilderState,
    path: &[usize],
    area: Rect,
    row_style: Style,
    flat_index: usize,
) {
    let Some(leaf) = state.leaf_at_mut(path) else {
        return;
    };

    // Layout: [Field▼][space][Op▼][space][value...][del]
    // Columns:  12       1    12    1      rest-1    8
    let [field_area, op_area, val_area, del_area] = area.layout(
        &Layout::horizontal([
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Min(2),
            Constraint::Length(8),
        ])
        .flex(Flex::SpaceBetween),
    );

    // Record leaf focus to use later (after releasing mutable borrow)
    let leaf_focus_field = leaf.leaf_focus == LeafFocus::Field;
    let leaf_focus_operator = leaf.leaf_focus == LeafFocus::Operator;
    let leaf_value = leaf.value.clone();
    let leaf_leaf_focus = leaf.leaf_focus;

    // Field
    let s = if leaf_focus_field {
        row_style.add_modifier(Modifier::REVERSED)
    } else {
        row_style
    };
    let field_dd = Dropdown::new().style(s);
    frame.render_stateful_widget(field_dd, field_area, &mut leaf.field_dd);

    // Operator
    let s = if leaf_focus_operator {
        row_style.add_modifier(Modifier::REVERSED)
    } else {
        row_style
    };
    let widget = Dropdown::new().style(s);
    frame.render_stateful_widget(widget, op_area, &mut leaf.operator_dd);

    // Drop the leaf borrow before we start mutating state.clickable_regions
    let _ = leaf;

    // Record clickable regions (now that we've dropped the leaf borrow)
    // Field
    state
        .clickable_regions
        .push(ClickableAction::LeafField.region(field_area, path.to_vec(), flat_index));
    // Operator
    state
        .clickable_regions
        .push(ClickableAction::LeafOperator.region(op_area, path.to_vec(), flat_index));
    // Value
    render_value_area(frame, &leaf_value, leaf_leaf_focus, val_area, row_style);
    // Record the clickable region for the value
    state
        .clickable_regions
        .push(ClickableAction::LeafValue.region(val_area, path.to_vec(), flat_index));

    // Del button
    let del_style = Style::default().fg((*TEXT_HIGHLIGHT).into());
    frame.render_widget(Line::from("[-Del]").style(del_style), del_area);
    // Record the clickable region for the delete button
    state.clickable_regions.push(ClickableAction::Delete.region(
        del_area,
        path.to_vec(),
        flat_index,
    ));
}

fn render_value_area(
    frame: &mut Frame<'_>,
    value: &UiValue,
    focus: LeafFocus,
    area: Rect,
    row_style: Style,
) {
    let is_focused = focus == LeafFocus::Value;
    let border_color: ratatui::style::Color = if is_focused {
        (*BORDER_FOCUSED).into()
    } else {
        (*BORDER_UNFOCUSED).into()
    };

    let s = if is_focused {
        row_style.add_modifier(Modifier::REVERSED)
    } else {
        row_style
    };
    match value {
        UiValue::Text(input) => {
            let display = if input.is_empty() { "value" } else { input };
            frame.render_widget(Line::from_iter(["\"", display, "\""]).style(s), area);
        }
        UiValue::Integer(input) => {
            let display = if input.is_empty() { "year" } else { input };
            frame.render_widget(Line::from(display).style(s), area);
        }
        UiValue::Set(items) => {
            frame.render_widget(
                items
                    .iter()
                    .map(|item| format!("[{item}]"))
                    .collect::<Line<'_>>()
                    .style(s),
                area,
            );
        }
    }
    let _ = border_color; // used only via style above
}
