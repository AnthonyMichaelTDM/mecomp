/**
This is a snippet from the `tui_tree_widget` crate, with modifications. The original source can be found at:
<https://github.com/EdJoPaTo/tui-rs-tree-widget/blob/b5fc5ca6938421bc83cf2e22e5c32846ac0a6413/src/flatten.rs>

the original source code is under the following license:

MIT License

Copyright (c) `EdJoPaTo`

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
use std::collections::HashSet;

use super::item::CheckTreeItem;

/// A flattened item of all visible [`CheckTreeItem`]s.
///
/// Generated via [`CheckTreeState::flatten`](super::CheckTreeState::flatten).
#[must_use]
pub struct Flattened<'text, Identifier> {
    pub identifier: Vec<Identifier>,
    pub item: &'text CheckTreeItem<'text, Identifier>,
}

impl<Identifier> Flattened<'_, Identifier> {
    /// Zero based depth. Depth 0 means top level with 0 indentation.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.identifier.len() - 1
    }
}

/// Get a flat list of all visible [`CheckTreeItem`]s.
///
/// `current` starts empty: `&[]`
#[must_use]
pub fn flatten<'text, Identifier, S: ::std::hash::BuildHasher>(
    open_identifiers: &HashSet<Vec<Identifier>, S>,
    items: &'text [CheckTreeItem<'text, Identifier>],
    current: &[Identifier],
) -> Vec<Flattened<'text, Identifier>>
where
    Identifier: Clone + PartialEq + Eq + core::hash::Hash,
{
    let mut result = Vec::new();
    for item in items {
        let mut child_identifier = current.to_vec();
        child_identifier.push(item.identifier.clone());

        let child_result = open_identifiers
            .contains(&child_identifier)
            .then(|| flatten(open_identifiers, &item.children, &child_identifier));

        result.push(Flattened {
            identifier: child_identifier,
            item,
        });

        if let Some(mut child_result) = child_result {
            result.append(&mut child_result);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_works() {
        let mut open = HashSet::new();
        open.insert(vec!["b"]);
        open.insert(vec!["b", "d"]);
        let depths = flatten(&open, &CheckTreeItem::example(), &[])
            .into_iter()
            .map(|flattened| flattened.depth())
            .collect::<Vec<_>>();
        assert_eq!(depths, [0, 0, 1, 1, 2, 2, 1, 0]);
    }

    #[cfg(test)]
    fn flatten_works(open: &HashSet<Vec<&'static str>>, expected: &[&str]) {
        let items = CheckTreeItem::example();
        let result = flatten(open, &items, &[]);
        let actual = result
            .into_iter()
            .map(|flattened| flattened.identifier.into_iter().next_back().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn flatten_nothing_open_is_top_level() {
        let open = HashSet::new();
        flatten_works(&open, &["a", "b", "h"]);
    }

    #[test]
    fn flatten_wrong_open_is_only_top_level() {
        let mut open = HashSet::new();
        open.insert(vec!["a"]);
        open.insert(vec!["b", "d"]);
        flatten_works(&open, &["a", "b", "h"]);
    }

    #[test]
    fn flatten_one_is_open() {
        let mut open = HashSet::new();
        open.insert(vec!["b"]);
        flatten_works(&open, &["a", "b", "c", "d", "g", "h"]);
    }

    #[test]
    fn flatten_all_open() {
        let mut open = HashSet::new();
        open.insert(vec!["b"]);
        open.insert(vec!["b", "d"]);
        flatten_works(&open, &["a", "b", "c", "d", "e", "f", "g", "h"]);
    }
}
