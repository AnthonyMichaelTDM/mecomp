//! implementation of the radio view

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Alignment,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    checktree_utils::{
        create_song_tree_leaf, get_checked_things_from_tree_state,
        get_selected_things_from_tree_state,
    },
    RadioViewProps,
};
use crate::{
    state::action::{Action, AudioAction, PopupAction, QueueAction},
    ui::{
        colors::{BORDER_FOCUSED, BORDER_UNFOCUSED, TEXT_HIGHLIGHT, TEXT_NORMAL},
        components::{Component, ComponentRender, RenderProps},
        widgets::{
            popups::PopupType,
            tree::{state::CheckTreeState, CheckTree},
        },
        AppState,
    },
};

#[allow(clippy::module_name_repetitions)]
pub struct RadioView {
    /// Action Sender
    pub action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    pub props: Option<RadioViewProps>,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
}

impl Component for RadioView {
    fn new(state: &AppState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx,
            props: state.additional_view_data.radio.clone(),
            tree_state: Mutex::new(CheckTreeState::default()),
        }
    }

    fn move_with_state(self, state: &AppState) -> Self
    where
        Self: Sized,
    {
        if let Some(props) = &state.additional_view_data.radio {
            Self {
                props: Some(props.to_owned()),
                ..self
            }
        } else {
            self
        }
    }

    fn name(&self) -> &str {
        "Radio"
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // arrow keys
            KeyCode::PageUp => {
                self.tree_state.lock().unwrap().select_relative(|current| {
                    current.map_or(
                        self.props
                            .as_ref()
                            .map_or(0, |p| p.songs.len().saturating_sub(1)),
                        |c| c.saturating_sub(10),
                    )
                });
            }
            KeyCode::Up => {
                self.tree_state.lock().unwrap().key_up();
            }
            KeyCode::PageDown => {
                self.tree_state
                    .lock()
                    .unwrap()
                    .select_relative(|current| current.map_or(0, |c| c.saturating_add(10)));
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
                    let things =
                        get_selected_things_from_tree_state(&self.tree_state.lock().unwrap());

                    if let Some(thing) = things {
                        self.action_tx
                            .send(Action::SetCurrentView(thing.into()))
                            .unwrap();
                    }
                }
            }
            // if there are checked items, send to queue, otherwise send whole radio to queue
            KeyCode::Char('q') => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(things))))
                        .unwrap();
                } else if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Audio(AudioAction::Queue(QueueAction::Add(
                            props.songs.iter().map(|s| s.id.clone().into()).collect(),
                        ))))
                        .expect("failed to send action");
                }
            }
            // if there are checked items, add to playlist, otherwise add whole radio to playlist
            KeyCode::Char('p') => {
                let things = get_checked_things_from_tree_state(&self.tree_state.lock().unwrap());
                if !things.is_empty() {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(
                            things,
                        ))))
                        .unwrap();
                } else if let Some(props) = &self.props {
                    self.action_tx
                        .send(Action::Popup(PopupAction::Open(PopupType::Playlist(
                            props.songs.iter().map(|s| s.id.clone().into()).collect(),
                        ))))
                        .expect("failed to send action");
                }
            }
            _ => {}
        }
    }
}

impl ComponentRender<RenderProps> for RadioView {
    fn render_border(&self, frame: &mut Frame, props: RenderProps) -> RenderProps {
        let border_style = if props.is_focused {
            Style::default().fg(BORDER_FOCUSED.into())
        } else {
            Style::default().fg(BORDER_UNFOCUSED.into())
        };

        let area = if let Some(state) = &self.props {
            let border = Block::bordered()
                .title_top(Line::from(vec![
                    Span::styled("Radio", Style::default().bold()),
                    Span::raw(" "),
                    Span::styled(format!("top {}", state.count), Style::default().italic()),
                ]))
                .title_bottom(" \u{23CE} : Open | ←/↑/↓/→: Navigate | \u{2423} Check")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            let content_area = border.inner(props.area);

            // create an additional border around the content area to display additional instructions
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top("q: add to queue | p: add to playlist")
                .border_style(border_style);
            frame.render_widget(&border, content_area);
            let content_area = border.inner(content_area);

            // draw an additional border around the content area to indicate whether operations will be performed on the entire item, or just the checked items
            let border = Block::default()
                .borders(Borders::TOP)
                .title_top(Line::from(vec![
                    Span::raw("Performing operations on "),
                    Span::raw(
                        if get_checked_things_from_tree_state(&self.tree_state.lock().unwrap())
                            .is_empty()
                        {
                            "entire radio"
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
                .title_top("Radio")
                .border_style(border_style);
            frame.render_widget(&border, props.area);
            border.inner(props.area)
        };

        RenderProps { area, ..props }
    }

    fn render_content(&self, frame: &mut Frame, props: RenderProps) {
        if let Some(state) = &self.props {
            // create a tree to hold the radio results
            let items = state
                .songs
                .iter()
                .map(|song| create_song_tree_leaf(song))
                .collect::<Vec<_>>();

            // render the radio results
            frame.render_stateful_widget(
                CheckTree::new(&items)
                    .unwrap()
                    .highlight_style(Style::default().fg(TEXT_HIGHLIGHT.into()).bold())
                    .experimental_scrollbar(Some(Scrollbar::new(
                        ScrollbarOrientation::VerticalRight,
                    ))),
                props.area,
                &mut self.tree_state.lock().unwrap(),
            );
        } else {
            let text = "Empty Radio";

            frame.render_widget(
                Line::from(text)
                    .style(Style::default().fg(TEXT_NORMAL.into()))
                    .alignment(Alignment::Center),
                props.area,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal, state_with_everything},
        ui::components::content_view::ActiveView,
    };
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_new() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = state_with_everything();
        let view = RadioView::new(&state, tx);

        assert_eq!(view.name(), "Radio");
        assert_eq!(view.props, Some(state.additional_view_data.radio.unwrap()));
    }

    #[test]
    fn test_move_with_state() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::default();
        let new_state = state_with_everything();
        let view = RadioView::new(&state, tx).move_with_state(&new_state);

        assert_eq!(
            view.props,
            Some(new_state.additional_view_data.radio.unwrap())
        );
    }

    #[test]
    fn test_render_empty() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = RadioView::new(&AppState::default(), tx);

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
            "┌Radio─────────┐",
            "│ Empty Radio  │",
            "└──────────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_render() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let view = RadioView::new(&state_with_everything(), tx);

        let (mut terminal, area) = setup_test_terminal(50, 6);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Radio top 1─────────────────────────────────────┐",
            "│q: add to queue | p: add to playlist────────────│",
            "│Performing operations on entire radio───────────│",
            "│☐ Test Song Test Artist                         │",
            "│                                                │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check─────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn test_render_with_checked() -> Result<()> {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = RadioView::new(&state_with_everything(), tx);
        let (mut terminal, area) = setup_test_terminal(50, 6);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Radio top 1─────────────────────────────────────┐",
            "│q: add to queue | p: add to playlist────────────│",
            "│Performing operations on entire radio───────────│",
            "│☐ Test Song Test Artist                         │",
            "│                                                │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check─────────┘",
        ]);
        assert_buffer_eq(&buffer, &expected);

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        let buffer = terminal
            .draw(|frame| view.render(frame, props))
            .unwrap()
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            "┌Radio top 1─────────────────────────────────────┐",
            "│q: add to queue | p: add to playlist────────────│",
            "│Performing operations on checked items──────────│",
            "│☑ Test Song Test Artist                         │",
            "│                                                │",
            "└ ⏎ : Open | ←/↑/↓/→: Navigate | ␣ Check─────────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);

        Ok(())
    }

    #[test]
    fn smoke_navigation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut view = RadioView::new(&state_with_everything(), tx);

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        view.handle_key_event(KeyEvent::from(KeyCode::PageUp));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
        view.handle_key_event(KeyEvent::from(KeyCode::Left));
        view.handle_key_event(KeyEvent::from(KeyCode::Right));
    }

    #[test]
    fn test_actions() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = RadioView::new(&state_with_everything(), tx);

        // need to render the view at least once to load the tree state
        let (mut terminal, area) = setup_test_terminal(50, 6);
        let props = RenderProps {
            area,
            is_focused: true,
        };
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // we test the actions when:
        // there are no checked items
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "song",
                item_id()
            )
                .into()])))
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "song",
                item_id()
            )
                .into()])))
        );

        // there are checked items
        // first we need to select an item (the album)
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        let _frame = terminal.draw(|frame| view.render(frame, props)).unwrap();

        // open the selected view
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::SetCurrentView(ActiveView::Song(item_id()))
        );

        // check the artist
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        // add to queue
        view.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Audio(AudioAction::Queue(QueueAction::Add(vec![(
                "song",
                item_id()
            )
                .into()])))
        );

        // add to playlist
        view.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(
            rx.blocking_recv().unwrap(),
            Action::Popup(PopupAction::Open(PopupType::Playlist(vec![(
                "song",
                item_id()
            )
                .into()])))
        );
    }
}
