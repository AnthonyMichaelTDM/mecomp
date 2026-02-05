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
    state::action::{Action, ViewAction},
    ui::{
        AppState,
        colors::{TEXT_HIGHLIGHT, TEXT_NORMAL, border_color},
        components::{
            Component, ComponentRender, RenderProps,
            content_view::views::traits::{SortMode, SortableViewProps},
        },
        widgets::tree::{CheckTree, state::CheckTreeState},
    },
};

use super::{
    ItemViewProps,
    checktree_utils::{
        construct_add_to_playlist_action, construct_add_to_queue_action,
        construct_start_radio_action,
    },
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
                props: Some(props),
                tree_state: Mutex::new(CheckTreeState::default()),
                ..self
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
                            .send(Action::ActiveView(ViewAction::Set(thing.into())))
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
            _ => {
                if let Some(props) = &mut self.props {
                    props.handle_extra_key_events(
                        key,
                        self.action_tx.clone(),
                        &mut self.tree_state.lock().unwrap(),
                    );
                }
            }
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));
        let [_, content_area] = Props::split_area(area);
        let footer = u16::from(Props::extra_footer().is_some());
        let content_area = Rect {
            y: content_area.y + 2,
            height: content_area.height - 2 - footer,
            ..content_area
        };

        let result = self
            .tree_state
            .lock()
            .unwrap()
            .handle_mouse_event(mouse, content_area, false);
        if let Some(action) = result {
            self.action_tx.send(action).unwrap();
        }
    }
}

impl<Props> ComponentRender<RenderProps> for ItemView<Props>
where
    Props: ItemViewProps,
{
    fn render_border(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

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
                            Props::none_checked_string()
                        } else {
                            "checked items"
                        },
                    )
                    .fg(*TEXT_HIGHLIGHT),
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

    fn render_content(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
        let Some(state) = &self.props else {
            let text = format!("No active {}", Props::name());

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg((*TEXT_NORMAL).into()))
                    .alignment(Alignment::Center),
                props.area,
            );
            return;
        };

        // create a tree to hold the items children
        let items = state.tree_items().unwrap();

        // render the tree
        frame.render_stateful_widget(
            CheckTree::new(&items)
                .unwrap()
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                .experimental_scrollbar(Props::scrollbar()),
            props.area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

/// Wraps an [`ItemView`] with sorting functionality
///
/// Defines additional key handling for changing sort modes,
///
/// Overrides rendering since we need to display the current sort mode in the border
#[derive(Debug)]
pub struct SortableItemView<Props, Mode, Item> {
    pub item_view: ItemView<Props>,
    pub sort_mode: Mode,
    _item: std::marker::PhantomData<Item>,
}

impl<Props, Mode, Item> Component for SortableItemView<Props, Mode, Item>
where
    Props: ItemViewProps + SortableViewProps<Item>,
    Mode: SortMode<Item>,
{
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let item_view = ItemView::new(state, action_tx);
        let sort_mode = Mode::default();
        Self {
            item_view,
            sort_mode,
            _item: std::marker::PhantomData,
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        let mut item_view = self.item_view.move_with_state(state);

        if let Some(props) = &mut item_view.props {
            props.sort_items(&self.sort_mode);
        }

        Self { item_view, ..self }
    }

    fn name(&self) -> &str {
        self.item_view.name()
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // Change sort mode
            crossterm::event::KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.next();
                if let Some(props) = &mut self.item_view.props {
                    props.sort_items(&self.sort_mode);
                    self.item_view
                        .tree_state
                        .lock()
                        .unwrap()
                        .scroll_selected_into_view();
                }
            }
            crossterm::event::KeyCode::Char('S') => {
                self.sort_mode = self.sort_mode.prev();
                if let Some(props) = &mut self.item_view.props {
                    props.sort_items(&self.sort_mode);
                    self.item_view
                        .tree_state
                        .lock()
                        .unwrap()
                        .scroll_selected_into_view();
                }
            }
            _ => self.item_view.handle_key_event(key),
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        self.item_view.handle_mouse_event(mouse, area);
    }
}

impl<Props, Mode, Item> ComponentRender<RenderProps> for SortableItemView<Props, Mode, Item>
where
    Props: ItemViewProps + SortableViewProps<Item>,
    Mode: SortMode<Item>,
{
    fn render_border(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) -> RenderProps {
        let border_style = Style::default().fg(border_color(props.is_focused).into());

        // draw borders and get area for content
        let area = if let Some(state) = &self.item_view.props {
            let border = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled(Props::title(), Style::default().bold()),
                    Span::raw(" sorted by: "),
                    Span::styled(self.sort_mode.to_string(), Style::default().italic()),
                ]))
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
                .title_top("q: add to queue | r: start radio | p: add to playlist")
                .border_style(border_style);
            let border = if let Some(extra_footer) = Props::extra_footer() {
                border
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .title_bottom(extra_footer)
            } else {
                border.border_style(border_style)
            };
            frame.render_widget(&border, content_area);
            let content_area = border.inner(content_area);

            // draw an additional border around the content area to indicate whether operations will be performed on the entire item, or just the checked items
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top(Line::from(vec![
                    Span::raw("Performing operations on "),
                    Span::raw(
                        if self
                            .item_view
                            .tree_state
                            .lock()
                            .unwrap()
                            .get_checked_things()
                            .is_empty()
                        {
                            Props::none_checked_string()
                        } else {
                            "checked items"
                        },
                    )
                    .fg(*TEXT_HIGHLIGHT),
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

    fn render_content(&mut self, frame: &mut ratatui::Frame<'_>, props: RenderProps) {
        self.item_view.render_content(frame, props);
    }
}
