//! Utility types and functions.

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

// implement an iterator over the OneOrMany type
impl<T: std::clone::Clone> IntoIterator for OneOrMany<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            OneOrMany::One(t) => vec![t].into_iter(),
            OneOrMany::Many(t) => t.into_iter(),
        }
    }
}
