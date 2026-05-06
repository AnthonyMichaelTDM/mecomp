use ratatui::{
    Frame,
    layout::{Offset, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::ui::{
    colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL},
    widgets::{dropdown::Dropdown, input_box::InputBox},
};

use super::{
    state::{BuilderMode, QueryBuilderState},
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
            (*BORDER_FOCUSED).into()
        } else {
            (*BORDER_UNFOCUSED).into()
        }
    } else {
        (*TEXT_HIGHLIGHT).into()
    };

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
    let mut scroll_offset = state.scroll_offset;

    // If cursor is above the visible area, scroll up to show it
    if cursor_idx < scroll_offset {
        scroll_offset = cursor_idx;
    }

    // If cursor is below the visible area, scroll down to show it
    if cursor_idx >= scroll_offset + visible_rows {
        scroll_offset = cursor_idx.saturating_sub(visible_rows - 1);
    }

    // Ensure we don't scroll past the end
    if n > visible_rows {
        scroll_offset = scroll_offset.min(n - visible_rows);
    } else {
        scroll_offset = 0;
    }

    // Update the stored scroll offset
    state.scroll_offset = scroll_offset;

    // render rows
    let mut row_y = inner.y;
    for (rel_i, node) in flat.iter().enumerate().skip(scroll_offset) {
        if row_y >= inner.y + area.height {
            break;
        }
        let row_area = Rect {
            x: inner.x,
            y: row_y,
            width: inner.width,
            height: 1,
        };
        let is_cursor = rel_i == cursor_idx;
        render_flat_node(frame, state, node, row_area, is_cursor, rel_i);
        row_y += 1;
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
    let inner_area = Rect {
        x: area.x.saturating_add(indent).min(area.x + area.width),
        width: area.width.saturating_sub(indent),
        ..area
    };

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
            state.clickable_regions.push(super::state::ClickableRegion {
                area: inner_area,
                action: super::state::ClickableAction::AddClause,
                path: node.path.clone(),
                flat_index,
                leaf_focus: None,
            });
        }
        FlatNodeKind::AddGroup => {
            let label = Span::styled("[+ Add Group]", cursor_style);
            frame.render_widget(Paragraph::new(Line::from(label)), inner_area);
            state.clickable_regions.push(super::state::ClickableRegion {
                area: inner_area,
                action: super::state::ClickableAction::AddGroup,
                path: node.path.clone(),
                flat_index,
                leaf_focus: None,
            });
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
    let kind_width = 8u16.min(area.width);
    let [kind_area, rest_area] = split2(area, kind_width);

    if kind_area.width > 0 {
        let kind_dd = Dropdown::new().style(row_style);
        frame.render_stateful_widget(kind_dd, area, &mut group.kind_dd);
        // Record the clickable region for the kind dropdown
        state.clickable_regions.push(super::state::ClickableRegion {
            area: kind_area,
            action: super::state::ClickableAction::GroupKind,
            path: path.to_vec(),
            flat_index,
            leaf_focus: None,
        });
    }

    // Del button if non-root
    if !path.is_empty() && rest_area.width >= 10 {
        let [_, del_area] = split2(rest_area, rest_area.width.saturating_sub(10));
        let del_style = Style::default().fg((*TEXT_HIGHLIGHT).into());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("[-Del]", del_style))),
            del_area,
        );
        // Record the clickable region for the delete button
        state.clickable_regions.push(super::state::ClickableRegion {
            area: del_area,
            action: super::state::ClickableAction::Delete,
            path: path.to_vec(),
            flat_index,
            leaf_focus: None,
        });
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
    let field_w = 14u16.min(area.width);
    let op_w = 14u16.min(area.width.saturating_sub(field_w + 2));
    let del_w = 8u16;
    let val_w = area
        .width
        .saturating_sub(field_w + 1 + op_w + 1 + del_w + 1);

    let [field_area, rest] = split2(area, field_w);
    let [_, rest] = split2(rest, 1); // gap
    let [op_area, rest] = split2(rest, op_w);
    let [_, rest] = split2(rest, 1); // gap
    let [val_area, rest] = split2(rest, val_w);
    let [_, del_area] = split2(rest, 1); // gap before del

    // Record leaf focus to use later (after releasing mutable borrow)
    let leaf_focus_field = leaf.leaf_focus == LeafFocus::Field;
    let leaf_focus_operator = leaf.leaf_focus == LeafFocus::Operator;
    let leaf_value = leaf.value.clone();
    let leaf_leaf_focus = leaf.leaf_focus;

    // Field
    if field_area.width > 0 {
        let s = if leaf_focus_field {
            row_style.add_modifier(Modifier::REVERSED)
        } else {
            row_style
        };
        let field_dd = Dropdown::new().style(s);
        frame.render_stateful_widget(field_dd, field_area, &mut leaf.field_dd);
    }

    // Operator
    if op_area.width > 0 {
        let s = if leaf_focus_operator {
            row_style.add_modifier(Modifier::REVERSED)
        } else {
            row_style
        };
        let widget = Dropdown::new().style(s);
        frame.render_stateful_widget(widget, op_area, &mut leaf.operator_dd);
    }

    // Drop the leaf borrow before we start mutating state.clickable_regions
    let _ = leaf;

    // Record clickable regions (now that we've dropped the leaf borrow)
    if field_area.width > 0 {
        state.clickable_regions.push(super::state::ClickableRegion {
            area: field_area,
            action: super::state::ClickableAction::LeafField,
            path: path.to_vec(),
            flat_index,
            leaf_focus: Some(LeafFocus::Field),
        });
    }

    if op_area.width > 0 {
        state.clickable_regions.push(super::state::ClickableRegion {
            area: op_area,
            action: super::state::ClickableAction::LeafOperator,
            path: path.to_vec(),
            flat_index,
            leaf_focus: Some(LeafFocus::Operator),
        });
    }

    // Value
    if val_area.width > 0 {
        render_value_area(frame, &leaf_value, leaf_leaf_focus, val_area, row_style);
        // Record the clickable region for the value
        state.clickable_regions.push(super::state::ClickableRegion {
            area: val_area,
            action: super::state::ClickableAction::LeafValue,
            path: path.to_vec(),
            flat_index,
            leaf_focus: Some(LeafFocus::Value),
        });
    }

    // Del button
    if del_area.width >= 6 {
        let del_style = Style::default().fg((*TEXT_HIGHLIGHT).into());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("[-Del]", del_style))),
            del_area,
        );
        // Record the clickable region for the delete button
        state.clickable_regions.push(super::state::ClickableRegion {
            area: del_area,
            action: super::state::ClickableAction::Delete,
            path: path.to_vec(),
            flat_index,
            leaf_focus: None,
        });
    }
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
        UiValue::Text(input) | UiValue::Integer(input) => {
            let label = if matches!(value, UiValue::Integer(_)) {
                "year"
            } else {
                "value"
            };
            // Inline display without a full border to keep row height = 1
            let text = input.text();
            let display: String = text.chars().take(area.width as usize).collect();
            let _ = label;
            frame.render_widget(Paragraph::new(Line::from(Span::styled(display, s))), area);
        }
        UiValue::Set { items, item_input } => {
            let mut parts: Vec<Span<'_>> = Vec::new();
            for item in items {
                parts.push(Span::styled(format!("[{item}×]"), s));
            }
            parts.push(Span::styled(item_input.text().to_string(), s));
            let line = Line::from(parts);
            frame.render_widget(Paragraph::new(line), area);
        }
    }
    let _ = border_color; // used only via style above
}

fn split2(area: Rect, left_width: u16) -> [Rect; 2] {
    let left_width = left_width.min(area.width);
    let right_width = area.width - left_width;
    [
        Rect {
            width: left_width,
            ..area
        },
        Rect {
            x: area.x + left_width,
            width: right_width,
            ..area
        },
    ]
}
