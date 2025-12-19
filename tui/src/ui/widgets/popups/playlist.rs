//! A popup that prompts the user to select a playlist, or create a new one.
//!
//! The popup will consist of an input box for the playlist name, a list of playlists to select from, and a button to create a new playlist.
//!
//! The user can navigate the list of playlists using the arrow keys, and select a playlist by pressing the enter key.
//!
//! The user can create a new playlist by typing a name in the input box and pressing the enter key.
//!
//! The user can cancel the popup by pressing the escape key.

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mecomp_prost::{RecordId, Ulid};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    state::action::{Action, LibraryAction, PopupAction},
    ui::{
        AppState,
        colors::{BORDER_FOCUSED, TEXT_HIGHLIGHT, TEXT_HIGHLIGHT_ALT},
        components::{
            Component, ComponentRender,
            content_view::views::{checktree_utils::create_playlist_tree_leaf, playlist::Props},
        },
        widgets::{
            input_box::{InputBox, RenderProps},
            tree::{CheckTree, state::CheckTreeState},
        },
    },
};

use super::Popup;

/// A popup that prompts the user to select a playlist, or create a new one.
///
/// The popup will consist of a list of playlists to select from,
/// and if the user wants to create a new playlist, they can press the "n" key,
/// which will make an input box appear for the user to type the name of the new playlist.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct PlaylistSelector {
    /// Action Sender
    action_tx: UnboundedSender<Action>,
    /// Mapped Props from state
    props: Props,
    /// tree state
    tree_state: Mutex<CheckTreeState<String>>,
    /// Playlist Name Input Box
    input_box: InputBox,
    /// Is the input box visible
    input_box_visible: bool,
    /// The items to add to the playlist
    items: Vec<RecordId>,
}

impl PlaylistSelector {
    #[must_use]
    pub fn new(state: &AppState, action_tx: UnboundedSender<Action>, items: Vec<RecordId>) -> Self {
        Self {
            input_box: InputBox::new(state, action_tx.clone()),
            input_box_visible: false,
            action_tx,
            props: Props::from(state),
            tree_state: Mutex::new(CheckTreeState::default()),
            items,
        }
    }
}

impl Popup for PlaylistSelector {
    fn title(&self) -> Line<'static> {
        Line::from("Select a Playlist")
    }

    fn instructions(&self) -> Line<'static> {
        if self.input_box_visible {
            Line::default()
        } else {
            Line::from("  \u{23CE} : Select | ↑/↓: Up/Down")
        }
    }

    fn update_with_state(&mut self, state: &AppState) {
        self.props = Props::from(state);
    }

    fn area(&self, terminal_area: Rect) -> Rect {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Min(31),
                Constraint::Percentage(19),
            ]);
        let [_, horizontal_area, _] = terminal_area.layout(&layout);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Max(10),
                Constraint::Min(10),
                Constraint::Max(10),
            ]);
        let [_, area, _] = horizontal_area.layout(&layout);
        area
    }

    fn inner_handle_key_event(&mut self, key: KeyEvent) {
        // this component has 2 distinct states:
        // 1. the user is selecting a playlist
        // 2. the user is creating a new playlist
        // when the user is creating a new playlist, the input box is visible
        // and the user can type the name of the new playlist
        // when the user is selecting a playlist, the input box is not visible
        // and the user can navigate the list of playlists
        if self.input_box_visible {
            match key.code {
                // if the user presses Enter, we try to create a new playlist with the given name
                // and add the items to that playlist
                KeyCode::Enter => {
                    let name = self.input_box.text();
                    if !name.is_empty() {
                        // create the playlist and add the items,
                        self.action_tx
                            .send(Action::Library(LibraryAction::CreatePlaylistAndAddThings(
                                name.to_string(),
                                self.items.clone(),
                            )))
                            .unwrap();
                        // close the popup
                        self.action_tx
                            .send(Action::Popup(PopupAction::Close))
                            .unwrap();
                    }
                    self.input_box_visible = false;
                }
                // defer to the input box
                _ => self.input_box.handle_key_event(key),
            }
        } else {
            match key.code {
                // if the user presses the "n" key, we show the input box
                KeyCode::Char('n') => {
                    self.input_box_visible = true;
                }
                // arrow keys
                KeyCode::PageUp => {
                    self.tree_state.lock().unwrap().select_relative(|current| {
                        current.map_or(self.props.playlists.len() - 1, |c| c.saturating_sub(10))
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
                // Enter key adds the items to the selected playlist
                // and closes the popup
                KeyCode::Enter => {
                    if self.tree_state.lock().unwrap().toggle_selected() {
                        let things = self.tree_state.lock().unwrap().get_selected_thing();

                        if let Some(thing) = things {
                            // add the items to the selected playlist
                            self.action_tx
                                .send(Action::Library(LibraryAction::AddThingsToPlaylist(
                                    thing,
                                    self.items.clone(),
                                )))
                                .unwrap();
                            // close the popup
                            self.action_tx
                                .send(Action::Popup(PopupAction::Close))
                                .unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Mouse Event Handler for the inner component of the popup,
    /// when an item in the list is clicked, it will be selected.
    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            kind, column, row, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        // adjust the area to account for the border
        let area = area.inner(Margin::new(1, 1));

        // defer to input box if it's visible
        if self.input_box_visible {
            let [input_box_area, content_area] = split_area(area);
            if input_box_area.contains(mouse_position) {
                self.input_box.handle_mouse_event(mouse, input_box_area);
            } else if content_area.contains(mouse_position)
                && kind == MouseEventKind::Down(MouseButton::Left)
            {
                self.input_box_visible = false;
            }
            return;
        }

        // if the mouse is outside the area, return
        if !area.contains(mouse_position) {
            return;
        }

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.tree_state.lock().unwrap().mouse_click(mouse_position);
            }
            MouseEventKind::ScrollDown => {
                self.tree_state.lock().unwrap().key_down();
            }
            MouseEventKind::ScrollUp => {
                self.tree_state.lock().unwrap().key_up();
            }
            _ => {}
        }
    }
}

fn split_area(area: Rect) -> [Rect; 2] {
    let [input_box_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .areas(area);
    [input_box_area, content_area]
}

impl ComponentRender<Rect> for PlaylistSelector {
    fn render_border(&self, frame: &mut ratatui::Frame<'_>, area: Rect) -> Rect {
        let area = self.render_popup_border(frame, area);

        let content_area = if self.input_box_visible {
            // split content area to make room for the input box
            let [input_box_area, content_area] = split_area(area);

            // render input box
            self.input_box.render(
                frame,
                RenderProps {
                    area: input_box_area,
                    text_color: (*TEXT_HIGHLIGHT_ALT).into(),
                    border: Block::bordered()
                        .title("Enter Name:")
                        .border_style(Style::default().fg((*BORDER_FOCUSED).into())),
                    show_cursor: self.input_box_visible,
                },
            );

            content_area
        } else {
            area
        };

        // draw additional border around content area to display additional instructions
        let border = Block::new()
            .borders(Borders::TOP)
            .title_top(if self.input_box_visible {
                " \u{23CE} : Create (cancel if empty)"
            } else {
                "n: new playlist"
            })
            .border_style(Style::default().fg(self.border_color()));
        frame.render_widget(&border, content_area);
        border.inner(content_area)
    }

    fn render_content(&self, frame: &mut Frame<'_>, area: Rect) {
        // create a tree for the playlists
        let playlists = self
            .props
            .playlists
            .iter()
            .map(create_playlist_tree_leaf)
            .collect::<Vec<_>>();

        // render the playlists
        frame.render_stateful_widget(
            CheckTree::new(&playlists)
                .unwrap()
                .highlight_style(Style::default().fg((*TEXT_HIGHLIGHT).into()).bold())
                // we want this to be rendered like a normal tree, not a check tree, so we don't show the checkboxes
                .node_unchecked_symbol("▪ ")
                .node_checked_symbol("▪ ")
                .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight))),
            area,
            &mut self.tree_state.lock().unwrap(),
        );
    }
}

/// Popup for changing the name of a playlist.
pub struct PlaylistEditor {
    action_tx: UnboundedSender<Action>,
    playlist_id: Ulid,
    input_box: InputBox,
}

impl PlaylistEditor {
    #[must_use]
    pub fn new(
        state: &AppState,
        action_tx: UnboundedSender<Action>,
        playlist_id: Ulid,
        playlist_name: &str,
    ) -> Self {
        let mut input_box = InputBox::new(state, action_tx.clone());
        input_box.set_text(playlist_name);

        Self {
            action_tx,
            playlist_id,
            input_box,
        }
    }
}

impl Popup for PlaylistEditor {
    fn title(&self) -> Line<'static> {
        Line::from("Rename Playlist")
    }

    fn instructions(&self) -> Line<'static> {
        Line::from(" \u{23CE} : Rename")
    }

    /// Should be located in the upper middle of the screen
    fn area(&self, terminal_area: Rect) -> Rect {
        let height = 5;
        let width = u16::try_from(
            self.input_box
                .text()
                .len()
                .max(self.instructions().width())
                .max(self.title().width())
                + 5,
        )
        .unwrap_or(terminal_area.width)
        .min(terminal_area.width);

        let x = (terminal_area.width - width) / 2;
        let y = (terminal_area.height - height) / 2;

        Rect::new(x, y, width, height)
    }

    fn update_with_state(&mut self, _: &AppState) {}

    fn inner_handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let name = self.input_box.text();
                if name.is_empty() {
                    return;
                }

                self.action_tx
                    .send(Action::Popup(PopupAction::Close))
                    .unwrap();
                self.action_tx
                    .send(Action::Library(LibraryAction::RenamePlaylist(
                        self.playlist_id.clone(),
                        name.to_string(),
                    )))
                    .unwrap();
            }
            _ => self.input_box.handle_key_event(key),
        }
    }

    fn inner_handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) {
        let MouseEvent {
            column, row, kind, ..
        } = mouse;
        let mouse_position = Position::new(column, row);

        if area.contains(mouse_position) {
            self.input_box.handle_mouse_event(mouse, area);
        } else if kind == MouseEventKind::Down(MouseButton::Left) {
            self.action_tx
                .send(Action::Popup(PopupAction::Close))
                .unwrap();
        }
    }
}

impl ComponentRender<Rect> for PlaylistEditor {
    fn render_border(&self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        self.render_popup_border(frame, area)
    }

    fn render_content(&self, frame: &mut Frame<'_>, area: Rect) {
        self.input_box.render(
            frame,
            RenderProps {
                area,
                text_color: (*TEXT_HIGHLIGHT_ALT).into(),
                border: Block::bordered()
                    .title("Enter Name:")
                    .border_style(Style::default().fg((*BORDER_FOCUSED).into())),
                show_cursor: true,
            },
        );
    }
}

#[cfg(test)]
mod selector_tests {
    use super::*;
    use crate::{
        state::component::ActiveComponent,
        test_utils::{item_id, setup_test_terminal},
        ui::components::content_view::{ActiveView, views::ViewData},
    };
    use anyhow::Result;
    use mecomp_core::{config::Settings, state::StateAudio};
    use mecomp_prost::{LibraryBrief, PlaylistBrief, SearchResult};
    use mecomp_storage::db::schemas::playlist::TABLE_NAME;
    use pretty_assertions::assert_eq;
    use ratatui::{
        buffer::Buffer,
        style::{Color, Style, Stylize},
        text::Span,
    };
    use rstest::{fixture, rstest};

    #[fixture]
    fn state() -> AppState {
        AppState {
            active_component: ActiveComponent::default(),
            audio: StateAudio::default(),
            search: SearchResult::default(),
            library: LibraryBrief {
                playlists: vec![PlaylistBrief {
                    id: RecordId::new(TABLE_NAME, item_id()),
                    name: "playlist 1".into(),
                }],
                ..Default::default()
            },
            active_view: ActiveView::default(),
            additional_view_data: ViewData::default(),
            settings: Settings::default(),
        }
    }

    #[fixture]
    fn border_style() -> Style {
        Style::reset().fg(Color::Rgb(3, 169, 244))
    }

    #[fixture]
    fn input_box_style() -> Style {
        Style::reset().fg(Color::Rgb(239, 154, 154))
    }

    #[rstest]
    #[case::large((100, 100), Rect::new(50, 10, 31, 80))]
    #[case::small((31, 10), Rect::new(0, 0, 31, 10))]
    #[case::too_small((20, 5), Rect::new(0, 0, 20, 5))]
    fn test_playlist_selector_area(
        #[case] terminal_size: (u16, u16),
        #[case] expected_area: Rect,
        state: AppState,
    ) {
        let (_, area) = setup_test_terminal(terminal_size.0, terminal_size.1);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let items = vec![];
        let area = PlaylistSelector::new(&state, action_tx, items).area(area);
        assert_eq!(area, expected_area);
    }

    #[rstest]
    fn test_playlist_selector_render(
        state: AppState,
        #[from(border_style)] style: Style,
    ) -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(31, 10);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let items = vec![];
        let popup = PlaylistSelector::new(&state, action_tx, items);
        let buffer = terminal
            .draw(|frame| popup.render_popup(frame))?
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            Line::styled("┌Select a Playlist────────────┐", style),
            Line::styled("│n: new playlist──────────────│", style),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("▪ "),
                Span::raw("playlist 1").bold(),
                Span::raw("                 "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::from(vec![
                Span::styled("│", style),
                Span::raw("                             "),
                Span::styled("│", style),
            ]),
            Line::styled("└  ⏎ : Select | ↑/↓: Up/Down──┘", style),
        ]);

        assert_eq!(buffer, expected);

        Ok(())
    }

    #[rstest]
    fn test_playlist_selector_render_input_box(
        state: AppState,
        border_style: Style,
        input_box_style: Style,
    ) -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(31, 10);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let items = vec![];
        let mut popup = PlaylistSelector::new(&state, action_tx, items);
        popup.inner_handle_key_event(KeyEvent::from(KeyCode::Char('n')));
        let buffer = terminal
            .draw(|frame| popup.render_popup(frame))?
            .buffer
            .clone();
        let expected = Buffer::with_lines([
            Line::styled("┌Select a Playlist────────────┐", border_style),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::styled("┌Enter Name:────────────────┐", input_box_style),
                Span::styled("│", border_style),
            ]),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::styled("│                           │", input_box_style),
                Span::styled("│", border_style),
            ]),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::styled("└───────────────────────────┘", input_box_style),
                Span::styled("│", border_style),
            ]),
            Line::styled("│ ⏎ : Create (cancel if empty)│", border_style),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::raw("▪ "),
                Span::raw("playlist 1").bold(),
                Span::raw("                 "),
                Span::styled("│", border_style),
            ]),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::raw("                             "),
                Span::styled("│", border_style),
            ]),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::raw("                             "),
                Span::styled("│", border_style),
            ]),
            Line::from(vec![
                Span::styled("│", border_style),
                Span::raw("                             "),
                Span::styled("│", border_style),
            ]),
            Line::styled("└─────────────────────────────┘", border_style),
        ]);

        assert_eq!(buffer, expected);

        Ok(())
    }
}

#[cfg(test)]
mod editor_tests {
    use super::*;
    use crate::{
        state::component::ActiveComponent,
        test_utils::{assert_buffer_eq, item_id, setup_test_terminal},
        ui::components::content_view::{ActiveView, views::ViewData},
    };
    use anyhow::Result;
    use mecomp_core::{config::Settings, state::StateAudio};
    use mecomp_prost::{LibraryBrief, PlaylistBrief, SearchResult};
    use mecomp_storage::db::schemas::playlist::TABLE_NAME;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use rstest::{fixture, rstest};

    #[fixture]
    fn state() -> AppState {
        AppState {
            active_component: ActiveComponent::default(),
            audio: StateAudio::default(),
            search: SearchResult::default(),
            library: LibraryBrief::default(),
            active_view: ActiveView::default(),
            additional_view_data: ViewData::default(),
            settings: Settings::default(),
        }
    }

    #[fixture]
    fn playlist() -> PlaylistBrief {
        PlaylistBrief {
            id: RecordId::new(TABLE_NAME, item_id()),
            name: "Test Playlist".into(),
        }
    }

    #[rstest]
    #[case::large((100, 100), Rect::new(40, 47, 20, 5))]
    #[case::small((20,5), Rect::new(0, 0, 20, 5))]
    #[case::too_small((10, 5), Rect::new(0, 0, 10, 5))]
    fn test_playlist_editor_area(
        #[case] terminal_size: (u16, u16),
        #[case] expected_area: Rect,
        state: AppState,
        playlist: PlaylistBrief,
    ) {
        let (_, area) = setup_test_terminal(terminal_size.0, terminal_size.1);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let editor = PlaylistEditor::new(&state, action_tx, playlist.id.ulid(), &playlist.name);
        let area = editor.area(area);
        assert_eq!(area, expected_area);
    }

    #[rstest]
    fn test_playlist_editor_render(state: AppState, playlist: PlaylistBrief) -> Result<()> {
        let (mut terminal, _) = setup_test_terminal(20, 5);
        let action_tx = tokio::sync::mpsc::unbounded_channel().0;
        let editor = PlaylistEditor::new(&state, action_tx, playlist.id.ulid(), &playlist.name);
        let buffer = terminal
            .draw(|frame| editor.render_popup(frame))?
            .buffer
            .clone();

        let expected = Buffer::with_lines([
            "┌Rename Playlist───┐",
            "│┌Enter Name:─────┐│",
            "││Test Playlist   ││",
            "│└────────────────┘│",
            "└ ⏎ : Rename───────┘",
        ]);

        assert_buffer_eq(&buffer, &expected);
        Ok(())
    }

    #[rstest]
    fn test_playlist_editor_input(state: AppState, playlist: PlaylistBrief) {
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut editor = PlaylistEditor::new(&state, action_tx, playlist.id.ulid(), &playlist.name);

        // Test typing
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(editor.input_box.text(), "Test Playlista");

        // Test enter with name
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(editor.input_box.text(), "Test Playlista");
        assert_eq!(
            action_rx.blocking_recv(),
            Some(Action::Popup(PopupAction::Close))
        );
        assert_eq!(
            action_rx.blocking_recv(),
            Some(Action::Library(LibraryAction::RenamePlaylist(
                playlist.id.into(),
                "Test Playlista".into()
            )))
        );

        // Test backspace
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(editor.input_box.text(), "Test Playlist");

        // Test enter with empty name
        editor.input_box.set_text("");
        editor.inner_handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(editor.input_box.text(), "");
    }
}
