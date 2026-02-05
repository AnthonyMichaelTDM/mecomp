//! an empty view

use ratatui::{layout::Alignment, style::Style, text::Line, widgets::Block};

use crate::ui::{
    AppState,
    colors::{TEXT_NORMAL, border_color},
    components::{Component, ComponentRender, RenderProps},
};

#[allow(clippy::module_name_repetitions)]
pub struct NoneView;

impl Component for NoneView {
    fn new(
        _state: &AppState,
        _action_tx: tokio::sync::mpsc::UnboundedSender<crate::state::action::Action>,
    ) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn move_with_state(self, _state: &AppState) -> Self
    where
        Self: Sized,
    {
        self
    }

    fn name(&self) -> &'static str {
        "None"
    }

    fn handle_key_event(&mut self, _key: crossterm::event::KeyEvent) {
        // do nothing
    }

    fn handle_mouse_event(&mut self, _: crossterm::event::MouseEvent, _: ratatui::layout::Rect) {
        // do nothing
    }
}

impl ComponentRender<RenderProps> for NoneView {
    fn render_border(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        let block = Block::bordered().border_style(border_style);
        let area = block.inner(props.area);
        frame.render_widget(block, props.area);

        RenderProps { area, ..props }
    }

    fn render_content(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
        let text = "No active view";

        frame.render_widget(
            Line::from(text)
                .style(Style::default().fg((*TEXT_NORMAL).into()))
                .alignment(Alignment::Center),
            props.area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use ratatui::buffer::Buffer;

    #[test]
    fn test_render() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = NoneView::new(&AppState::default(), tx).move_with_state(&AppState {
            active_view: ActiveView::None,
            ..state_with_everything()
        });

        let (mut terminal, area) = setup_test_terminal(16, 3);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌──────────────┐",
            "│No active view│",
            "└──────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
    }
}
