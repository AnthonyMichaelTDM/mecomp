use crate::OneOrMany;

#[allow(clippy::module_name_repetitions)]
pub struct OneOrManyIter<'a, T> {
    pub(crate) inner: &'a OneOrMany<T>,
    pub(crate) index: usize,
}

impl<'a, T> Iterator for OneOrManyIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.inner {
            OneOrMany::One(t) => {
                if self.index == 0 {
                    Some(t)
                } else {
                    None
                }
            }
            OneOrMany::Many(t) => t.get(self.index),
            OneOrMany::None => None,
        };
        self.index += 1;
        result
    }
}

impl<'a, T> IntoIterator for &'a OneOrMany<T> {
    type IntoIter = OneOrManyIter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> OneOrMany<T> {
    /// Returns an iterator over the values in the `OneOrMany`.
    pub const fn iter(&self) -> OneOrManyIter<T> {
        OneOrManyIter {
            inner: self,
            index: 0,
        }
    }
}

impl<T: Clone> FromIterator<T> for OneOrMany<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter: <I as IntoIterator>::IntoIter = iter.into_iter();
        let mut result: Self = Self::None;
        for item in iter {
            result.push(item);
        }
        result
    }
}
