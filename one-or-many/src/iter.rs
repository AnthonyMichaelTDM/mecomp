use crate::OneOrMany;

impl<T: ToOwned<Owned = T>> FromIterator<T> for OneOrMany<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut result: Self = Self::None;
        result.extend(iter);
        result
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct Iter<'a, T> {
    inner: &'a OneOrMany<T>,
    index: usize,
}

impl<T> OneOrMany<T> {
    /// Returns an iterator over the values in the `OneOrMany`.
    #[inline]
    #[must_use]
    pub const fn iter(&self) -> Iter<T> {
        Iter {
            inner: self,
            index: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a OneOrMany<T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.inner.get(self.index);
        self.index += 1;
        result
    }
}

/// A consuming iterator over the values in a `OneOrMany`.
///
/// This is really just a wrapper around other iterators.
///
/// TODO: find a better way to do this
#[allow(clippy::module_name_repetitions)]
pub struct IntoIter<T> {
    inner_iter: InnerIntoIter<T>,
}

enum InnerIntoIter<T> {
    One(Option<T>),
    Many(std::vec::IntoIter<T>),
    None,
}

impl<T> IntoIterator for OneOrMany<T> {
    type IntoIter = IntoIter<T>;
    type Item = T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let inner_iter = match self {
            Self::One(t) => InnerIntoIter::One(Some(*t)),
            Self::Many(v) => InnerIntoIter::Many(v.into_iter()),
            Self::None => InnerIntoIter::None,
        };

        IntoIter { inner_iter }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner_iter {
            InnerIntoIter::One(ref mut t) => t.take(),
            InnerIntoIter::Many(ref mut v) => v.next(),
            InnerIntoIter::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::OneOrMany;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case::none(Vec::<usize>::new().into_iter(), OneOrMany::<usize>::None)]
    #[case::one(vec![1].into_iter(), OneOrMany::from(1))]
    #[case::many(vec![1,2,3].into_iter(), OneOrMany::Many(vec![1,2,3]))]
    fn test_from_iter<T, I>(#[case] input: I, #[case] expected: OneOrMany<T>)
    where
        T: std::fmt::Debug + Clone + std::cmp::PartialEq,
        I: Iterator<Item = T>,
    {
        let collected = input.collect::<OneOrMany<_>>();
        assert_eq!(collected, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, vec![None])]
    #[case::one(OneOrMany::from(1), vec![Some(1), None])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), vec![Some(1), Some(2), Some(3), None])]
    fn test_iter<T>(#[case] input: OneOrMany<T>, #[case] expected: Vec<Option<T>>)
    where
        T: std::fmt::Debug + Clone + std::cmp::PartialEq,
    {
        let mut iter = input.iter();

        for item in expected {
            let next = iter.next();
            assert_eq!(next, item.as_ref());
        }
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, vec![None])]
    #[case::one(OneOrMany::from(1), vec![Some(1), None])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), vec![Some(1), Some(2), Some(3), None])]
    fn test_into_iter_byval<T>(#[case] input: OneOrMany<T>, #[case] expected: Vec<Option<T>>)
    where
        T: std::fmt::Debug + Clone + std::cmp::PartialEq,
    {
        let mut iter = input.into_iter();

        for item in expected {
            let next = iter.next();
            assert_eq!(next, item);
        }
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None)]
    #[case::one(OneOrMany::from(1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]))]
    fn test_for_loop<T>(#[case] input: OneOrMany<T>)
    where
        T: std::fmt::Debug + Clone + std::cmp::PartialEq,
    {
        // non-consuming
        let mut iter = input.iter();
        for item in &input {
            assert_eq!(iter.next(), Some(item));
        }
        assert_eq!(iter.next(), None);

        // consuming
        let mut iter = input.clone().into_iter();
        for item in input {
            assert_eq!(iter.next(), Some(item));
        }
        assert_eq!(iter.next(), None);
    }
}
