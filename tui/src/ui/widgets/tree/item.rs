use std::collections::HashSet;

use ratatui::text::Text;

/// One item inside a [`CheckTree`](super::CheckTree).
///
/// Can have zero or more `children`.
///
/// # Identifier
///
/// The generic argument `Identifier` is used to keep the state like the currently selected or opened [`CheckTreeItem`]s in the [`CheckTreeState`](super::state::CheckTreeState).
///
/// It needs to be unique among its siblings but can be used again on parent or child [`CheckTreeItem`]s.
/// A common example would be a filename which has to be unique in its directory while it can exist in another.
///
/// The `text` can be different from its `identifier`.
/// To repeat the filename analogy: File browsers sometimes hide file extensions.
/// The filename `main.rs` is the identifier while its shown as `main`.
/// Two files `main.rs` and `main.toml` can exist in the same directory and can both be displayed as `main` but their identifier is different.
///
/// Just like every file in a file system can be uniquely identified with its file and directory names each [`CheckTreeItem`] in a [`CheckTree`](super::CheckTree) can be with these identifiers.
/// As an example the following two identifiers describe the main file in a Rust cargo project: `vec!["src", "main.rs"]`.
#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct CheckTreeItem<'text, Identifier> {
    pub(super) identifier: Identifier,
    pub(super) text: Text<'text>,
    pub(super) children: Vec<Self>,
}

impl<'text, Identifier> CheckTreeItem<'text, Identifier>
where
    Identifier: Clone + PartialEq + Eq + core::hash::Hash,
{
    /// Create a new `CheckTreeItem` without children.
    #[must_use]
    pub fn new_leaf<T>(identifier: Identifier, text: T) -> Self
    where
        T: Into<Text<'text>>,
    {
        Self {
            identifier,
            text: text.into(),
            children: Vec::new(),
        }
    }

    /// Create a new `CheckTreeItem` with children.
    ///
    /// # Errors
    ///
    /// Errors when there are duplicate identifiers in the children.
    pub fn new<T>(identifier: Identifier, text: T, children: Vec<Self>) -> std::io::Result<Self>
    where
        T: Into<Text<'text>>,
    {
        let identifiers = children
            .iter()
            .map(|item| &item.identifier)
            .collect::<HashSet<_>>();
        if identifiers.len() != children.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "The children contain duplicate identifiers",
            ));
        }

        Ok(Self {
            identifier,
            text: text.into(),
            children,
        })
    }

    /// Get a reference to the identifier.
    #[must_use]
    pub const fn identifier(&self) -> &Identifier {
        &self.identifier
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // TODO: make this const
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    /// Get a reference to a child by index.
    #[must_use]
    pub fn child(&self, index: usize) -> Option<&Self> {
        self.children.get(index)
    }

    /// Get a mutable reference to a child by index.
    ///
    /// When you choose to change the `identifier` the [`CheckTreeState`](super::CheckTreeState) might not work as expected afterwards.
    #[must_use]
    pub fn child_mut(&mut self, index: usize) -> Option<&mut Self> {
        self.children.get_mut(index)
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.text.height()
    }

    /// Add a child to the `CheckTreeItem`.
    ///
    /// # Errors
    ///
    /// Errors when the `identifier` of the `child` already exists in the children.
    pub fn add_child(&mut self, child: Self) -> std::io::Result<()> {
        let existing = self
            .children
            .iter()
            .map(|item| &item.identifier)
            .collect::<HashSet<_>>();
        if existing.contains(&child.identifier) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "identifier already exists in the children",
            ));
        }

        self.children.push(child);
        Ok(())
    }
}

#[cfg(test)]
impl CheckTreeItem<'static, &'static str> {
    #[must_use]
    pub(crate) fn example() -> Vec<Self> {
        vec![
            Self::new_leaf("a", "Alfa"),
            Self::new(
                "b",
                "Bravo",
                vec![
                    Self::new_leaf("c", "Charlie"),
                    Self::new(
                        "d",
                        "Delta",
                        vec![Self::new_leaf("e", "Echo"), Self::new_leaf("f", "Foxtrot")],
                    )
                    .expect("all item identifiers are unique"),
                    Self::new_leaf("g", "Golf"),
                ],
            )
            .expect("all item identifiers are unique"),
            Self::new_leaf("h", "Hotel"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic = "duplicate identifiers"]
    fn tree_item_new_errors_with_duplicate_identifiers() {
        let item = CheckTreeItem::new_leaf("same", "text");
        let another = item.clone();
        CheckTreeItem::new("root", "Root", vec![item, another]).unwrap();
    }

    #[test]
    #[should_panic = "identifier already exists"]
    fn tree_item_add_child_errors_with_duplicate_identifiers() {
        let item = CheckTreeItem::new_leaf("same", "text");
        let another = item.clone();
        let mut root = CheckTreeItem::new("root", "Root", vec![item]).unwrap();
        root.add_child(another).unwrap();
    }
}
