use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    layout::{Alignment, Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::Action,
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_NORMAL},
        components::{Component, ComponentRender, RenderProps},
        widgets::tree::{state::CheckTreeState, CheckTree},
        AppState,
    },
};

use super::{
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        construct_start_radio_action,
    },
    ItemViewProps,
};

#[derive(Debug)]
pub struct ItemView<Props> {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<Props>,
    /// tree state
    pub tree_state: Mutex<CheckTreeState<String>>,
}

impl<Props> Component for ItemView<Props>
where
    Props: ItemViewProps,
{
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let props = Props::retrieve(&state.additional_view_data);
        let tree_state = Mutex::new(CheckTreeState::default());
        Self {
            action_tx,
            props,
            tree_state,
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = Props::retrieve(&state.additional_view_data) {
            Self {
                action_tx: self.action_tx,
                props: Some(props),
                tree_state: self.tree_state,
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        Props::title()
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::Up => {
                self.tree_state.lock().unwrap().key_up();
            }
            KeyCode::Down => {
                self.tree_state.lock().unwrap().key_down();
            }
            KeyCode::Left => {
                self.tree_state.lock().unwrap().key_left();
            }
            KeyCode::Right => {
                self.tree_state.lock().unwrap().key_right();
            }
            KeyCode::Char(' ') => {
                self.tree_state.lock().unwrap().key_space();
            }
            // Enter key opens selected view
            KeyCode::Enter => {
                if self.tree_state.lock().unwrap().toggle_selected() {
                    let things = self.tree_state.lock().unwrap().get_selected_thing();

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, add them to the queue, otherwise send the song to the queue
            KeyCode::Char('q') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_add_to_queue_action(
                    checked_things,
                    self.props.as_ref().map(super::ItemViewProps::id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, start radio from checked items, otherwise start radio from song
            KeyCode::Char('r') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_start_radio_action(
                    checked_things,
                    self.props.as_ref().map(super::ItemViewProps::id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            // if there are checked items, add them to playlist, otherwise add the song to playlist
            KeyCode::Char('p') => {
                let checked_things = self.tree_state.lock().unwrap().get_checked_things();
                if let Some(action) = construct_add_to_playlist_action(
                    checked_things,
                    self.props.as_ref().map(super::ItemViewProps::id),
                ) {
                    self.action_tx.send(action).unwrap();
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));
        let [_, content_area] = Props::split_area(area);
        let content_area = Rect {
            y: content_area.y + 2,
            height: content_area.height - 2,
            ..content_area
        };

        let result = self
            .tree_state
            .lock()
            .unwrap()
            .handle_mouse_event(mouse, content_area);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

impl<Props> ComponentRender<RenderProps> for ItemView<Props>
where
    Props: ItemViewProps,
{
    fn render_border(&self, frame: &mut ratatui::Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        // draw borders and get area for content
        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top(Props::title())
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            let content_area = border.inner(props.area);
            frame.render_widget(border, props.area);

            // split area to make room for item info
            let [info_area, content_area] = Props::split_area(content_area);

            // render item info
            frame.render_widget(state.info_widget(), info_area);

            // draw an additional border around the content area to display additional instructions
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top("q: add to queue | r: start radio | p: add to playlist")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            let content_area = border.inner(content_area);

            // draw an additional border around the content area to indicate whether operations will be performed on the entire item, or just the checked items
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top(Line::from(vec![
                    Span::raw("Performing operations on "),
                    Span::raw(
                        if self
                            .tree_state
                            .lock()
                            .unwrap()
                            .get_checked_things()
                            .is_empty()
                        {
                            Props::none_selected_string()
                        } else {
                            "checked items"
                        },
                    )
                    .fg(TEXT_HIGHLIGHT),
                ]))
                .italic()
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            border.inner(content_area)
        } else {
            let border = Block::bordered()
                .title_top(Props::title())
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create a tree to hold the items children
            let items = state.tree_items().unwrap();

            // render the tree
            frame.render_stateful_widget(
                CheckTree::new(&items)
                    .unwrap()
                    .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold()),
                props.area,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            let text = format!("No active {}", Props::name());

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}
