use ratatui::{
    Frame,
    layout::{Offset, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::ui::{
    colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT, TEXT_NORMAL},
    widgets::input_box::InputBox,
};

use super::state::{BuilderMode, ControlKind, ControlRef, QueryBuilderState};

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
                "Raw(r: visual mode)",
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
                "Visual(r: raw mode)",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]))
        .border_style(Style::default().fg(focused_color))
        .title_bottom("↑/↓/←/→: focus | Enter: open | a: add | g: group | d/Del: remove");

    let inner = border.inner(area);
    frame.render_widget(border, area);

    let focused = state.focused;
    let row_data = state
        .root_conditions()
        .map(|condition| {
            let field = condition.field.selected().unwrap_or("<field>").to_string();
            let op = condition.operator.selected().unwrap_or("<op>").to_string();
            let value = if condition.value.is_empty() {
                "<value>".to_string()
            } else {
                condition.value.clone()
            };
            (condition.id, field, op, value)
        })
        .collect::<Vec<_>>();

    state.clear_control_areas();

    let mut row_y = inner.y;
    for (condition_id, field, op, value) in row_data {
        if row_y >= inner.bottom() {
            break;
        }

        let row = Rect::new(inner.x, row_y, inner.width, 1);

        let field_w = 16.min(row.width);
        let op_w = 12.min(row.width.saturating_sub(field_w));
        let value_w = row.width.saturating_sub(field_w + op_w);

        let field_rect = Rect::new(row.x, row.y, field_w, 1);
        let op_rect = Rect::new(row.x + field_w, row.y, op_w, 1);
        let value_rect = Rect::new(row.x + field_w + op_w, row.y, value_w, 1);

        let field_control = ControlRef {
            condition_id,
            kind: ControlKind::Field,
        };
        let op_control = ControlRef {
            condition_id,
            kind: ControlKind::Operator,
        };
        let value_control = ControlRef {
            condition_id,
            kind: ControlKind::Value,
        };

        state.set_control_area(field_control, field_rect);
        state.set_control_area(op_control, op_rect);
        state.set_control_area(value_control, value_rect);

        frame.render_widget(
            render_cell(&field, focused == Some(field_control)),
            field_rect,
        );
        frame.render_widget(render_cell(&op, focused == Some(op_control)), op_rect);
        frame.render_widget(
            render_cell(&value, focused == Some(value_control)),
            value_rect,
        );

        row_y = row_y.saturating_add(1);
    }
}

fn render_cell(text: &str, focused: bool) -> Paragraph<'_> {
    let style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    Paragraph::new(Line::from(Span::styled(format!(" {text} "), style)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_render_query_builder() {
        todo!();
    }
}
