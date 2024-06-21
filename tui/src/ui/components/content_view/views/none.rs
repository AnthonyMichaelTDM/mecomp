//! an empty view

use ratatui::{layout::Alignment, style::Style, text::Line, widgets::Block};

use crate::ui::{
    colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_NORMAL},
    components::{Component, ComponentRender, RenderProps},
    AppState,
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

    fn name(&self) -> &str {
        "None"
    }

    fn handle_key_event(&mut self, _key: crossterm::event::KeyEvent) {
        // do nothing
    }
}

impl ComponentRender<RenderProps> for NoneView {
    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let block = Block::bordered().border_style(border_style);
        let area = block.inner(props.area);
        frame.render_widget(block, props.area);

        let text = "No active view";

        frame.render_widget(
            Line::from(text)
                .style(Style::default().fg(TEXT_NORMAL.into()))
                .alignment(Alignment::Center),
            area,
        );
    }
}
