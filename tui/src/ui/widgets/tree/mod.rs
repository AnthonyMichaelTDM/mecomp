pub mod flatten;
pub mod item;
pub mod state;

use flatten::Flattened;
use item::CheckTreeItem;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use state::CheckTreeState;
use unicode_width::UnicodeWidthStr;

/// A `CheckTree` which can be rendered.
///
/// The generic argument `Identifier` is used to keep the state like the currently selected or opened [`CheckTreeItem`]s in the [`CheckTreeState`].
/// For more information see [`CheckTreeItem`].
///
/// This differs from the `tui_tree_widget` crate's `Tree` in that it allows for checkboxes to be rendered next to each leaf item.
/// This is useful for creating a tree of items that can be selected.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct CheckTree<'a, Identifier> {
    items: &'a [CheckTreeItem<'a, Identifier>],

    block: Option<Block<'a>>,
    scrollbar: Option<Scrollbar<'a>>,
    /// Style used as a base style for the widget
    style: Style,

    /// Style used to render selected item
    highlight_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    highlight_symbol: &'a str,

    /// Symbol displayed in front of a closed node (As in the children are currently not visible)
    node_closed_symbol: &'a str,
    /// Symbol displayed in front of an open node. (As in the children are currently visible)
    node_open_symbol: &'a str,
    /// Symbol displayed in front of a node without children, that is checked
    node_checked_symbol: &'a str,
    /// Symbol displayed in front of a node without children, that is not checked
    node_unchecked_symbol: &'a str,

    _identifier: std::marker::PhantomData<Identifier>,
}

impl<'a, Identifier> CheckTree<'a, Identifier>
where
    Identifier: Clone + PartialEq + Eq + core::hash::Hash,
{
    /// Create a new `CheckTree`.
    ///
    /// # Errors
    ///
    /// Errors when there are duplicate identifiers in the children.
    pub fn new(items: &'a [CheckTreeItem<'a, Identifier>]) -> Result<Self, std::io::Error> {
        let identifiers = items
            .iter()
            .map(|item| &item.identifier)
            .collect::<std::collections::HashSet<_>>();
        if identifiers.len() != items.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "duplicate identifiers",
            ));
        }

        Ok(Self {
            items,
            block: None,
            scrollbar: None,
            style: Style::new(),
            highlight_style: Style::new(),
            highlight_symbol: "",
            node_closed_symbol: "\u{25b6} ", // ▸ Arrow to right (alt. ▸ U+25B8 BLACK RIGHT-POINTING SMALL TRIANGLE)
            node_open_symbol: "\u{25bc} ", // ▼ Arrow down (alt. ▾ U+25BE BLACK DOWN-POINTING SMALL TRIANGLE)
            node_checked_symbol: "\u{2611} ", // ☑ U+2611 BALLOT BOX WITH CHECK
            node_unchecked_symbol: "\u{2610} ", // ☐ U+2610 BALLOT BOX
            _identifier: std::marker::PhantomData,
        })
    }

    #[allow(clippy::missing_const_for_fn)] // false positive
    #[must_use]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Show the scrollbar when rendering this widget.
    ///
    /// Experimental: Can change on any release without any additional notice.
    /// Its there to test and experiment with whats possible with scrolling widgets.
    /// Also see <https://github.com/ratatui-org/ratatui/issues/174>
    #[must_use]
    pub const fn experimental_scrollbar(mut self, scrollbar: Option<Scrollbar<'a>>) -> Self {
        self.scrollbar = scrollbar;
        self
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub const fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    #[must_use]
    pub const fn highlight_symbol(mut self, highlight_symbol: &'a str) -> Self {
        self.highlight_symbol = highlight_symbol;
        self
    }

    #[must_use]
    pub const fn node_closed_symbol(mut self, symbol: &'a str) -> Self {
        self.node_closed_symbol = symbol;
        self
    }

    #[must_use]
    pub const fn node_open_symbol(mut self, symbol: &'a str) -> Self {
        self.node_open_symbol = symbol;
        self
    }

    #[must_use]
    pub const fn node_checked_symbol(mut self, symbol: &'a str) -> Self {
        self.node_checked_symbol = symbol;
        self
    }

    #[must_use]
    pub const fn node_unchecked_symbol(mut self, symbol: &'a str) -> Self {
        self.node_unchecked_symbol = symbol;
        self
    }
}

impl<'a, Identifier: 'a + Clone + PartialEq + Eq + core::hash::Hash> StatefulWidget
    for CheckTree<'a, Identifier>
{
    type State = CheckTreeState<Identifier>;

    #[allow(clippy::too_many_lines)]
    fn render(self, full_area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(full_area, self.style);

        // Get the inner area inside a possible block, otherwise use the full area
        let area = self.block.map_or(full_area, |block| {
            let inner_area = block.inner(full_area);
            block.render(full_area, buf);
            inner_area
        });

        state.last_area = area;
        state.last_rendered_identifiers.clear();
        if area.width < 1 || area.height < 1 {
            return;
        }

        let visible = state.flatten(self.items);
        state.last_biggest_index = visible.len().saturating_sub(1);
        if visible.is_empty() {
            return;
        }
        let available_height = area.height as usize;

        let ensure_index_in_view =
            if state.ensure_selected_in_view_on_next_render && !state.selected.is_empty() {
                visible
                    .iter()
                    .position(|flattened| flattened.identifier == state.selected)
            } else {
                None
            };

        // Ensure last line is still visible
        let mut start = state.offset.min(state.last_biggest_index);

        if let Some(ensure_index_in_view) = ensure_index_in_view {
            start = start.min(ensure_index_in_view);
        }

        let mut end = start;
        let mut height = 0;
        for item_height in visible
            .iter()
            .skip(start)
            .map(|flattened| flattened.item.height())
        {
            if height + item_height > available_height {
                break;
            }
            height += item_height;
            end += 1;
        }

        if let Some(ensure_index_in_view) = ensure_index_in_view {
            while ensure_index_in_view >= end {
                height += visible[end].item.height();
                end += 1;
                while height > available_height {
                    height = height.saturating_sub(visible[start].item.height());
                    start += 1;
                }
            }
        }

        state.offset = start;
        state.ensure_selected_in_view_on_next_render = false;

        let blank_symbol = " ".repeat(self.highlight_symbol.width());

        let mut current_height = 0;
        let has_selection = !state.selected.is_empty();
        #[allow(clippy::cast_possible_truncation)]
        for flattened in visible.iter().skip(state.offset).take(end - start) {
            let Flattened { identifier, item } = flattened;

            let x = area.x;
            let y = area.y + current_height;
            let height = item.height() as u16;
            current_height += height;

            let area = Rect {
                x,
                y,
                width: area.width,
                height,
            };

            let text = &item.text;
            let item_style = text.style;

            let is_selected = state.selected == *identifier;
            let after_highlight_symbol_x = if has_selection {
                let symbol = if is_selected {
                    self.highlight_symbol
                } else {
                    &blank_symbol
                };
                let (x, _) = buf.set_stringn(x, y, symbol, area.width as usize, item_style);
                x
            } else {
                x
            };

            let after_depth_x = {
                let indent_width = flattened.depth() * 2;
                let (after_indent_x, _) = buf.set_stringn(
                    after_highlight_symbol_x,
                    y,
                    " ".repeat(indent_width),
                    indent_width,
                    item_style,
                );
                let symbol = if text.width() == 0 {
                    "  "
                } else if item.children.is_empty() {
                    if state.checked.contains(identifier) {
                        self.node_checked_symbol
                    } else {
                        self.node_unchecked_symbol
                    }
                } else if state.opened.contains(identifier) {
                    self.node_open_symbol
                } else {
                    self.node_closed_symbol
                };
                let max_width = area.width.saturating_sub(after_indent_x - x);
                let (x, _) =
                    buf.set_stringn(after_indent_x, y, symbol, max_width as usize, item_style);
                x
            };

            let text_area = Rect {
                x: after_depth_x,
                width: area.width.saturating_sub(after_depth_x - x),
                ..area
            };
            text.render(text_area, buf);

            if is_selected {
                buf.set_style(area, self.highlight_style);
            }

            state
                .last_rendered_identifiers
                .push((area.y, identifier.clone()));
        }

        // render scrollbar last so it's on top
        if let Some(scrollbar) = self.scrollbar {
            let mut scrollbar_state = ScrollbarState::new(visible.len().saturating_sub(height))
                .position(start)
                .viewport_content_length(height);
            let scrollbar_area = Rect {
                // Inner height to be exactly as the content
                y: area.y,
                height: area.height,
                // Outer width to stay on the right border
                x: full_area.x,
                width: full_area.width,
            };
            scrollbar.render(scrollbar_area, buf, &mut scrollbar_state);
        }

        // update state
        state.last_identifiers = visible
            .into_iter()
            .map(|flattened| flattened.identifier)
            .collect();
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::{layout::Position, widgets::ScrollbarOrientation};

    #[must_use]
    #[track_caller]
    fn render(width: u16, height: u16, state: &mut CheckTreeState<&'static str>) -> Buffer {
        let items = CheckTreeItem::example();
        let tree = CheckTree::new(&items).unwrap();
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        StatefulWidget::render(tree, area, &mut buffer, state);
        buffer
    }

    #[test]
    #[should_panic = "duplicate identifiers"]
    fn tree_new_errors_with_duplicate_identifiers() {
        let item = CheckTreeItem::new_leaf("same", "text");
        let another = item.clone();
        let items = [item, another];
        let _: CheckTree<_> = CheckTree::new(&items).unwrap();
    }

    #[test]
    fn does_not_panic() {
        _ = render(0, 0, &mut CheckTreeState::default());
        _ = render(10, 0, &mut CheckTreeState::default());
        _ = render(0, 10, &mut CheckTreeState::default());
        _ = render(10, 10, &mut CheckTreeState::default());
    }

    #[test]
    fn scrollbar_renders_over_tree() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        let items = CheckTreeItem::example();
        let tree = CheckTree::new(&items)
            .unwrap()
            .experimental_scrollbar(Some(Scrollbar::new(ScrollbarOrientation::VerticalRight)));
        let area = Rect::new(0, 0, 10, 4);
        let mut buffer = Buffer::empty(area);
        StatefulWidget::render(tree, area, &mut buffer, &mut state);

        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☐ Alfa   ▲",
            "▼ Bravo  █",
            "  ☐ Charl█",
            "  ▶ Delta▼",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn renders_border() {
        let mut state = CheckTreeState::default();
        let items = CheckTreeItem::example();
        let tree = CheckTree::new(&items).unwrap().block(Block::bordered());
        let area = Rect::new(0, 0, 10, 5);
        let mut buffer = Buffer::empty(area);
        StatefulWidget::render(tree, area, &mut buffer, &mut state);

        let expected = Buffer::with_lines([
            "┌────────┐",
            "│☐ Alfa  │",
            "│▶ Bravo │",
            "│☐ Hotel │",
            "└────────┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn nothing_open() {
        let buffer = render(10, 4, &mut CheckTreeState::default());
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☐ Alfa    ",
            "▶ Bravo   ",
            "☐ Hotel   ",
            "          ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn check_leaf_d1() {
        let mut state = CheckTreeState::default();
        state.check(vec!["a"]);
        let buffer = render(10, 4, &mut state);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☑ Alfa    ",
            "▶ Bravo   ",
            "☐ Hotel   ",
            "          ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn check_parent_d1() {
        let mut state = CheckTreeState::default();
        state.check(vec!["b"]);
        let buffer = render(10, 4, &mut state);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☐ Alfa    ",
            "▶ Bravo   ",
            "☐ Hotel   ",
            "          ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn check_leaf_d2() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        state.check(vec!["b", "c"]);
        state.check(vec!["b", "g"]);
        let buffer = render(13, 7, &mut state);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☐ Alfa       ",
            "▼ Bravo      ",
            "  ☑ Charlie  ",
            "  ▶ Delta    ",
            "  ☑ Golf     ",
            "☐ Hotel      ",
            "             ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn depth_one() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        let buffer = render(13, 7, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa       ",
            "▼ Bravo      ",
            "  ☐ Charlie  ",
            "  ▶ Delta    ",
            "  ☐ Golf     ",
            "☐ Hotel      ",
            "             ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn depth_two() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        state.open(vec!["b", "d"]);
        let buffer = render(15, 9, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
            "    ☐ Echo     ",
            "    ☐ Foxtrot  ",
            "  ☐ Golf       ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(buffer, expected);
    }

    // TODO: test CheckTreeState::select_relative, rendered_at
    // key_up, key_down, key_left, key_right, and key_space

    #[test]
    fn test_select_first_last() {
        let mut state = CheckTreeState::default();
        let _ = render(15, 4, &mut state);
        assert_eq!(state.select_first(), true);
        assert_eq!(state.select_first(), false);
        assert_eq!(state.selected(), &["a"]);
        assert_eq!(state.select_last(), true);
        assert_eq!(state.select_last(), false);
        assert_eq!(state.selected(), &["h"]);
    }

    #[test]
    fn test_scroll_selected_into_view() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        state.open(vec!["b", "d"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // selected is visible
        state.select(vec!["b", "d"]);
        state.scroll_selected_into_view();
        let buffer = render(15, 4, &mut state);
        assert_eq!(buffer, expected);

        // selected is not visible
        state.select(vec!["b", "g"]);
        state.scroll_selected_into_view();
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "  ▼ Delta      ",
            "    ☐ Echo     ",
            "    ☐ Foxtrot  ",
            "  ☐ Golf       ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_scroll() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        state.open(vec!["b", "d"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // scroll down works
        assert_eq!(state.scroll_down(1), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
            "    ☐ Echo     ",
        ]);
        assert_eq!(buffer, expected);

        // scroll down stops at boundary
        assert_eq!(state.scroll_down(15), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Hotel        ",
            "               ",
            "               ",
            "               ",
        ]);
        assert_eq!(buffer, expected);

        // scroll down stops at boundary
        assert_eq!(state.scroll_down(1), false);

        // scroll up works
        assert_eq!(state.scroll_up(1), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "  ☐ Golf       ",
            "☐ Hotel        ",
            "               ",
            "               ",
        ]);
        assert_eq!(buffer, expected);

        // scroll up stops at boundary
        assert_eq!(state.scroll_up(15), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // scroll up stops at boundary
        assert_eq!(state.scroll_up(1), false);
    }

    #[test]
    fn test_keys() {
        let mut state = CheckTreeState::default();
        state.open(vec!["b"]);
        state.open(vec!["b", "d"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // key_down works (if nothing selected, goes to top)
        state.key_down();
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(state.selected(), &["a"]);
        assert_eq!(buffer, expected);

        // key_up works (if nothing selected, goes to bottom)
        state.key_left();
        state.key_up();
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "    ☐ Echo     ",
            "    ☐ Foxtrot  ",
            "  ☐ Golf       ",
            "☐ Hotel        ",
        ]);
        assert_eq!(state.selected(), &["h"]);
        assert_eq!(buffer, expected);

        // key_left works
        state.select_first();
        state.scroll_selected_into_view();
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(state.selected(), &["a"]);
        assert_eq!(buffer, expected);

        state.key_down();
        assert_eq!(state.selected(), &["b"]);
        state.key_left();
        assert_eq!(state.selected(), &["b"]);
        let buffer = render(15, 4, &mut state);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▶ Bravo        ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(buffer, expected);

        // key_right works
        state.key_right();
        assert_eq!(state.selected(), &["b"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // key_space works
        state.key_space();
        assert_eq!(state.selected(), &["b"]);
        state.key_down();
        state.key_space();
        assert_eq!(state.selected(), &["b", "c"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☑ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_rendered_at() {
        let mut state = CheckTreeState::default();

        // nothing rendered
        assert_eq!(state.rendered_at(Position::new(0, 0)), None);

        // render the tree
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▶ Bravo        ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(buffer, expected);

        // check rendered at
        assert_eq!(
            state.rendered_at(Position::new(0, 0)),
            Some(["a"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 1)),
            Some(["b"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 2)),
            Some(["h"].as_slice())
        );
        assert_eq!(state.rendered_at(Position::new(0, 3)), None);

        // open branch, render, and check again
        state.open(vec!["b"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▶ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        assert_eq!(
            state.rendered_at(Position::new(0, 0)),
            Some(["a"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 1)),
            Some(["b"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 2)),
            Some(["b", "c"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 3)),
            Some(["b", "d"].as_slice())
        );

        // close branch, render, and check again
        state.close(["b"].as_slice());
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▶ Bravo        ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(buffer, expected);

        assert_eq!(
            state.rendered_at(Position::new(0, 0)),
            Some(["a"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 1)),
            Some(["b"].as_slice())
        );
        assert_eq!(
            state.rendered_at(Position::new(0, 2)),
            Some(["h"].as_slice())
        );
        assert_eq!(state.rendered_at(Position::new(0, 3)), None);
    }

    #[test]
    fn test_mouse() {
        let mut state = CheckTreeState::default();

        // click before rendering
        assert_eq!(state.mouse_click(Position::new(0, 0)), false);

        state.open(vec!["b"]);
        state.open(vec!["b", "d"]);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(buffer, expected);

        // click on first item
        // check that the item is checked
        assert_eq!(state.mouse_click(Position::new(0, 0)), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☑ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(state.selected(), &["a"]);
        assert_eq!(buffer, expected);

        // click on second item
        // check that the branch is closed
        assert_eq!(state.mouse_click(Position::new(0, 1)), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☑ Alfa         ",
            "▶ Bravo        ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(state.selected(), &["b"]);
        assert_eq!(buffer, expected);

        // click on the first item again
        // check that the item is unchecked
        assert_eq!(state.mouse_click(Position::new(0, 0)), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▶ Bravo        ",
            "☐ Hotel        ",
            "               ",
        ]);
        assert_eq!(state.selected(), &["a"]);
        assert_eq!(buffer, expected);

        // click on empty space
        assert_eq!(state.mouse_click(Position::new(0, 4)), false);

        // click on the second item again
        // check that the branch is opened
        assert_eq!(state.mouse_click(Position::new(0, 1)), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☐ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(state.selected(), &["b"]);
        assert_eq!(buffer, expected);

        // click on the third item
        // check that the item is checked
        assert_eq!(state.mouse_click(Position::new(0, 2)), true);
        let buffer = render(15, 4, &mut state);
        let expected = Buffer::with_lines([
            "☐ Alfa         ",
            "▼ Bravo        ",
            "  ☑ Charlie    ",
            "  ▼ Delta      ",
        ]);
        assert_eq!(state.selected(), &["b", "c"]);
        assert_eq!(buffer, expected);
    }
}
