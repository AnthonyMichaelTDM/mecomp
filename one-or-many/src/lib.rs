#![deny(clippy::missing_inline_in_public_items)]

mod iter;
pub use iter::Iter;
#[cfg(feature = "surrealdb")]
mod query_result_impl;

use std::{
    clone::Clone,
    ops::{Index, IndexMut},
    slice::SliceIndex,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A type that can be either one value, many values, or no values.
///
/// Especially useful when working with deserialing data
///
/// To let it be useful in other contexts, it aims to implement many of the same traits and functions as `Vec<T>` and `Option<T>`.
#[derive(Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
    #[default]
    None,
}

impl<T> OneOrMany<T> {
    /// Returns the number of elements in the `OneOrMany`.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(t) => t.len(),
            Self::None => 0,
        }
    }

    /// Returns `true` if the `OneOrMany` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the value at the given index, or `None` if the index is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        match self {
            Self::One(t) if index == 0 => Some(t),
            Self::One(_) | Self::None => None,
            Self::Many(t) => t.get(index),
        }
    }

    /// Returns the first value, or `None` if the `OneOrMany` is empty.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        match self {
            Self::One(t) => Some(t),
            Self::Many(v) => v.first(),
            Self::None => None,
        }
    }

    /// Returns `true` if the `OneOrMany` contains the given value.
    #[inline]
    pub fn contains(&self, genre: &T) -> bool
    where
        T: PartialEq,
    {
        match self {
            Self::One(t) => t == genre,
            Self::Many(t) => t.contains(genre),
            Self::None => false,
        }
    }

    /// Pushes a new value onto the end of the `OneOrMany`.
    #[inline]
    pub fn push(&mut self, new: T)
    where
        T: ToOwned<Owned = T>,
    {
        match self {
            Self::One(t) => {
                *self = Self::Many(vec![t.to_owned(), new]);
            }
            Self::Many(t) => t.push(new),
            Self::None => *self = Self::One(new),
        }
    }

    /// Pops a value from the end of the `OneOrMany`.
    #[inline]
    pub fn pop(&mut self) -> Option<T>
    where
        T: ToOwned<Owned = T>,
    {
        match self {
            Self::One(t) => {
                let old = t.to_owned();
                *self = Self::None;
                Some(old)
            }
            Self::Many(t) => {
                let old = t.pop();
                if t.len() == 1 {
                    *self = Self::One(t[0].to_owned());
                }
                old
            }
            Self::None => None,
        }
    }

    /// Checks if the `OneOrMany` is `None`.
    #[inline]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Checks if the `OneOrMany` is `One`.
    #[inline]
    pub const fn is_one(&self) -> bool {
        matches!(self, Self::One(_))
    }

    /// Checks if the `OneOrMany` is `Many`.
    #[inline]
    pub const fn is_many(&self) -> bool {
        matches!(self, Self::Many(_))
    }

    /// Checks if the `OneOrMany` is `One` or `Many`.
    #[inline]
    pub const fn is_some(&self) -> bool {
        self.is_one() || self.is_many()
    }

    /// Gets a slice of the `OneOrMany`.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(t) => std::slice::from_ref(t),
            Self::Many(t) => t,
            Self::None => &[],
        }
    }

    /// Gets a mutable slice of the `OneOrMany`.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Self::One(t) => std::slice::from_mut(t),
            Self::Many(t) => t,
            Self::None => &mut [],
        }
    }

    /// Convert a `&OneOrMany<T>` to an `OneOrMany<&T>`
    ///
    /// Note, this will unfortunately cause an allocation if the `OneOrMany` is `Many`
    #[inline]
    pub fn as_ref(&self) -> OneOrMany<&T> {
        match *self {
            Self::One(ref x) => OneOrMany::One(x),
            Self::Many(ref v) => OneOrMany::Many(v.iter().collect()),
            Self::None => OneOrMany::None,
        }
    }

    /// remove duplicates from the `OneOrMany`
    ///
    /// internally converts to a `HashSet` and back
    #[inline]
    pub fn dedup(&mut self)
    where
        T: Clone + Eq + std::hash::Hash,
    {
        let mut set = std::collections::HashSet::new();
        let mut new = Vec::new();
        for t in self.as_slice() {
            if set.insert(t) {
                new.push(t.clone());
            }
        }
        *self = Self::from(new);
    }

    /// remove duplicates from the `OneOrMany` by some key
    ///
    /// internally converts to a `HashSet` and back
    #[inline]
    pub fn dedup_by_key<F, K>(&mut self, mut key: F)
    where
        F: FnMut(&T) -> K,
        K: Eq + std::hash::Hash,
        T: Clone,
    {
        let mut set = std::collections::HashSet::new();
        let mut new = Vec::new();
        for t in self.as_slice() {
            let key = key(t);
            if set.insert(key) {
                new.push(t.to_owned());
            }
        }
        *self = Self::from(new);
    }
}

impl<T: Clone> Clone for OneOrMany<T> {
    #[inline]
    fn clone(&self) -> Self {
        match self {
            Self::One(t) => Self::One(t.clone()),
            Self::Many(t) => Self::Many(t.clone()),
            Self::None => Self::None,
        }
    }
}

impl<T> From<T> for OneOrMany<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::One(t)
    }
}

impl<T> From<Option<T>> for OneOrMany<T> {
    #[inline]
    fn from(t: Option<T>) -> Self {
        t.map_or_else(|| Self::None, |t| Self::One(t))
    }
}

impl<T> From<Option<Vec<T>>> for OneOrMany<T> {
    #[inline]
    fn from(t: Option<Vec<T>>) -> Self {
        t.map_or_else(|| Self::None, Into::into)
    }
}

impl<T> From<Option<Self>> for OneOrMany<T> {
    #[inline]
    fn from(t: Option<Self>) -> Self {
        t.map_or_else(|| Self::None, |t| t)
    }
}

impl<T: Clone> From<&[T]> for OneOrMany<T> {
    #[inline]
    fn from(t: &[T]) -> Self {
        if t.is_empty() {
            Self::None
        } else if t.len() == 1 {
            Self::One(t[0].clone())
        } else {
            Self::Many(t.into())
        }
    }
}

impl<T> From<Vec<T>> for OneOrMany<T> {
    #[inline]
    fn from(t: Vec<T>) -> Self {
        if t.len() <= 1 {
            t.into_iter().next().map_or(Self::None, Self::One)
        } else {
            Self::Many(t)
        }
    }
}

impl<T> From<OneOrMany<T>> for Vec<T> {
    #[inline]
    fn from(value: OneOrMany<T>) -> Self {
        match value {
            OneOrMany::One(one) => vec![one],
            OneOrMany::Many(many) => many,
            OneOrMany::None => vec![],
        }
    }
}

// implement index traits by delegating to the slice
impl<T, I: SliceIndex<[T]>> Index<I> for OneOrMany<T> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(self.as_slice(), index)
    }
}
impl<T, I: SliceIndex<[T]>> IndexMut<I> for OneOrMany<T> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(self.as_mut_slice(), index)
    }
}

// implement partial ord
// None < One < Many
impl<T> PartialOrd<Self> for OneOrMany<T>
where
    T: PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::One(t1), Self::One(t2)) => t1.partial_cmp(t2),
            (Self::Many(t1), Self::Many(t2)) => t1.partial_cmp(t2),
            (Self::None, Self::None) => Some(std::cmp::Ordering::Equal),
            (Self::None, _) => Some(std::cmp::Ordering::Less),
            (_, Self::None) => Some(std::cmp::Ordering::Greater),
            (Self::One(_), _) => Some(std::cmp::Ordering::Less),
            (_, Self::One(_)) => Some(std::cmp::Ordering::Greater),
        }
    }
}

// implement ord
// None < One < Many
impl<T> Ord for OneOrMany<T>
where
    T: Ord,
{
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::One(t1), Self::One(t2)) => t1.cmp(t2),
            (Self::Many(t1), Self::Many(t2)) => t1.cmp(t2),
            (Self::None, Self::None) => std::cmp::Ordering::Equal,
            (Self::None, _) => std::cmp::Ordering::Less,
            (_, Self::None) => std::cmp::Ordering::Greater,
            (Self::One(_), _) => std::cmp::Ordering::Less,
            (_, Self::One(_)) => std::cmp::Ordering::Greater,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};
    use rstest::rstest;

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, 0)]
    #[case::one(OneOrMany::One(1), 1)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 3)]
    fn test_len<T>(#[case] input: OneOrMany<T>, #[case] expected: usize) {
        let actual = input.len();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, true)]
    #[case::one(OneOrMany::One(1), false)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), false)]
    fn test_is_empty<T>(#[case] input: OneOrMany<T>, #[case] expected: bool) {
        let actual = input.is_empty();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None,0, None)]
    #[case::none(OneOrMany::<usize>::None,1, None)]
    #[case::one(OneOrMany::One(1), 0, Some(&1))]
    #[case::one(OneOrMany::One(1), 1, None)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0, Some(&1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1, Some(&2))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 2, Some(&3))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 3, None)]
    fn test_get<T>(#[case] input: OneOrMany<T>, #[case] index: usize, #[case] expected: Option<&T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.get(index);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, None)]
    #[case::one(OneOrMany::One(1), Some(&1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), Some(&1))]
    fn test_first<T>(#[case] input: OneOrMany<T>, #[case] expected: Option<&T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.first();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, 2, false)]
    #[case::one(OneOrMany::One(1), 1, true)]
    #[case::one(OneOrMany::One(1), 0, false)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]),2, true)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]),4, false)]
    fn test_contains<T>(#[case] input: OneOrMany<T>, #[case] value: T, #[case] expected: bool)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.contains(&value);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, 1, OneOrMany::One(1))]
    #[case::one(OneOrMany::One(1), 2, OneOrMany::Many(vec![1, 2]))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 4, OneOrMany::Many(vec![1, 2, 3, 4]))]
    fn test_push<T>(#[case] mut input: OneOrMany<T>, #[case] new: T, #[case] expected: OneOrMany<T>)
    where
        T: Clone + PartialEq + std::fmt::Debug,
    {
        input.push(new);
        assert_eq!(input, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, None, OneOrMany::<usize>::None)]
    #[case::one(OneOrMany::One(1), Some(1), OneOrMany::<usize>::None)]
    #[case::many(OneOrMany::Many(vec![1, 2]), Some(2), OneOrMany::One(1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), Some(3), OneOrMany::Many(vec![1, 2]))]
    fn test_pop<T>(
        #[case] mut input: OneOrMany<T>,
        #[case] expected: Option<T>,
        #[case] expected_output: OneOrMany<T>,
    ) where
        T: Clone + PartialEq + std::fmt::Debug,
    {
        let result = input.pop();
        assert_eq!(result, expected);
        assert_eq!(input, expected_output);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, true)]
    #[case::one(OneOrMany::One(1), false)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), false)]
    fn test_is_none<T>(#[case] input: OneOrMany<T>, #[case] expected: bool)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.is_none();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, false)]
    #[case::one(OneOrMany::One(1), true)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), false)]
    fn test_is_one<T>(#[case] input: OneOrMany<T>, #[case] expected: bool)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.is_one();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, false)]
    #[case::one(OneOrMany::One(1), false)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), true)]
    fn test_is_many<T>(#[case] input: OneOrMany<T>, #[case] expected: bool)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.is_many();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, false)]
    #[case::one(OneOrMany::One(1), true)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), true)]
    fn test_is_some<T>(#[case] input: OneOrMany<T>, #[case] expected: bool)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.is_some();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, vec![])]
    #[case::one(OneOrMany::One(1), vec![1])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), vec![1, 2, 3])]
    fn test_as_slice<T>(#[case] input: OneOrMany<T>, #[case] expected: Vec<T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        assert_eq!(input.as_slice(), expected.as_slice());
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, vec![])]
    #[case::one(OneOrMany::One(1), vec![1])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), vec![1, 2, 3])]
    fn test_as_mut_slice<T>(#[case] mut input: OneOrMany<T>, #[case] mut expected: Vec<T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        assert_eq!(input.as_mut_slice(), expected.as_mut_slice());
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<&usize>::None)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(&1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![&1, &2, &3]))]
    fn test_as_ref<T>(#[case] input: OneOrMany<T>, #[case] expected: OneOrMany<&T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = input.as_ref();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::one(1, OneOrMany::One(1))]
    fn test_from<T>(#[case] input: T, #[case] expected: OneOrMany<T>)
    where
        T: Clone + PartialEq + std::fmt::Debug,
    {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(vec![], OneOrMany::<usize>::None)]
    #[case::one(vec![1], OneOrMany::One(1))]
    #[case::many(vec![1, 2, 3], OneOrMany::Many(vec![1, 2, 3]))]
    fn test_from_vec<T>(#[case] input: Vec<T>, #[case] expected: OneOrMany<T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(&[], OneOrMany::<usize>::None)]
    #[case::one(&[1], OneOrMany::One(1))]
    #[case::many(&[1, 2, 3], OneOrMany::Many(vec![1, 2, 3]))]
    fn test_from_slice<T>(#[case] input: &[T], #[case] expected: OneOrMany<T>)
    where
        T: PartialEq + std::fmt::Debug + Clone,
    {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(None, OneOrMany::<usize>::None)]
    #[case::one(Some(1), OneOrMany::One(1))]
    fn test_from_option(#[case] input: Option<usize>, #[case] expected: OneOrMany<usize>) {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(None, OneOrMany::<usize>::None)]
    #[case::one(Some(OneOrMany::One(1)), OneOrMany::One(1))]
    #[case::many(Some(OneOrMany::Many(vec![1, 2, 3])), OneOrMany::Many(vec![1, 2, 3]))]
    fn test_from_option_self(
        #[case] input: Option<OneOrMany<usize>>,
        #[case] expected: OneOrMany<usize>,
    ) {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(Option::<Vec<usize>>::None, OneOrMany::None)]
    #[case::one(Some(vec![1]), OneOrMany::One(1))]
    #[case::many(Some(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]))]
    fn test_from_option_vec(#[case] input: Option<Vec<usize>>, #[case] expected: OneOrMany<usize>) {
        let actual = OneOrMany::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, vec![])]
    #[case::one(OneOrMany::One(1), vec![1])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), vec![1, 2, 3])]
    fn test_into_vec(#[case] input: OneOrMany<usize>, #[case] expected: Vec<usize>) {
        let actual = Vec::from(input);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[should_panic = "index out of bounds: the len is 0 but the index is 0"]
    #[case::none(OneOrMany::<usize>::None, 0, 0)]
    #[case::one(OneOrMany::One(1), 0, 1)]
    #[should_panic = "index out of bounds: the len is 1 but the index is 1"]
    #[case::one(OneOrMany::One(1), 1, 0)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0, 1)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1, 2)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 2, 3)]
    #[should_panic = "index out of bounds: the len is 3 but the index is 3"]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 3, 4)]
    fn test_index<T>(#[case] input: OneOrMany<T>, #[case] index: usize, #[case] expected: T)
    where
        T: PartialEq + std::fmt::Debug + Copy,
    {
        let actual = input[index];
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, 0..0, &[])]
    #[should_panic = "range end index 1 out of range for slice of length 0"]
    #[case::none(OneOrMany::<usize>::None, 0..1, &[])]
    #[case::one(OneOrMany::One(1), 0..0, &[])]
    #[case::one(OneOrMany::One(1), 0..1, &[1])]
    #[should_panic = "range end index 2 out of range for slice of length 1"]
    #[case::one(OneOrMany::One(1), 1..2, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..0, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..1, &[1])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1..2, &[2])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 2..3, &[3])]
    #[should_panic = "range end index 4 out of range for slice of length 3"]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 3..4, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..3, &[1, 2, 3])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..2, &[1, 2])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1..3, &[2, 3])]
    fn test_index_slice<'a, T, I>(
        #[case] input: OneOrMany<T>,
        #[case] index: I,
        #[case] expected: &[T],
    ) where
        T: PartialEq + std::fmt::Debug,
        I: std::slice::SliceIndex<[T], Output = [T]>,
    {
        let actual = &input[index];
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, 0..0, &[])]
    #[should_panic = "range end index 1 out of range for slice of length 0"]
    #[case::none(OneOrMany::<usize>::None, 0..1, &[])]
    #[case::one(OneOrMany::One(1), 0..0, &[])]
    #[case::one(OneOrMany::One(1), 0..1, &[1])]
    #[should_panic = "range end index 2 out of range for slice of length 1"]
    #[case::one(OneOrMany::One(1), 1..2, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..0, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..1, &[1])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1..2, &[2])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 2..3, &[3])]
    #[should_panic = "range end index 4 out of range for slice of length 3"]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 3..4, &[])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..3, &[1, 2, 3])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 0..2, &[1, 2])]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), 1..3, &[2, 3])]
    fn test_index_mut_slice<'a, T, I>(
        #[case] mut input: OneOrMany<T>,
        #[case] index: I,
        #[case] expected: &[T],
    ) where
        T: PartialEq + std::fmt::Debug,
        I: std::slice::SliceIndex<[T], Output = [T]>,
    {
        let actual = &mut input[index];
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<usize>::None, Some(std::cmp::Ordering::Equal))]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::One(1), Some(std::cmp::Ordering::Less))]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::Many(vec![1, 2, 3]), Some(std::cmp::Ordering::Less))]
    #[case::one(OneOrMany::One(1), OneOrMany::<usize>::None, Some(std::cmp::Ordering::Greater))]
    #[case::one(OneOrMany::One(1), OneOrMany::One(1), Some(std::cmp::Ordering::Equal))]
    #[case::one(OneOrMany::One(1), OneOrMany::One(2), Some(std::cmp::Ordering::Less))]
    #[case::one(
        OneOrMany::One(1),
        OneOrMany::One(0),
        Some(std::cmp::Ordering::Greater)
    )]
    #[case::one(OneOrMany::One(1), OneOrMany::Many(vec![1, 2, 3]), Some(std::cmp::Ordering::Less))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::<usize>::None, Some(std::cmp::Ordering::Greater))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::One(1), Some(std::cmp::Ordering::Greater))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]), Some(std::cmp::Ordering::Equal))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![2, 3]), Some(std::cmp::Ordering::Less))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3, 4]), Some(std::cmp::Ordering::Less))]
    fn test_partial_cmp<T>(
        #[case] input: OneOrMany<T>,
        #[case] other: OneOrMany<T>,
        #[case] expected: Option<std::cmp::Ordering>,
    ) where
        T: std::fmt::Debug + PartialOrd,
    {
        let actual = input.partial_cmp(&other);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<usize>::None, std::cmp::Ordering::Equal)]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::One(1), std::cmp::Ordering::Less)]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::Many(vec![1, 2, 3]), std::cmp::Ordering::Less)]
    #[case::one(OneOrMany::One(1), OneOrMany::<usize>::None, std::cmp::Ordering::Greater)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(1), std::cmp::Ordering::Equal)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(2), std::cmp::Ordering::Less)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(0), std::cmp::Ordering::Greater)]
    #[case::one(OneOrMany::One(1), OneOrMany::Many(vec![1, 2, 3]), std::cmp::Ordering::Less)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::<usize>::None, std::cmp::Ordering::Greater)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::One(1), std::cmp::Ordering::Greater)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]), std::cmp::Ordering::Equal)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![2, 3]), std::cmp::Ordering::Less)]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3, 4]), std::cmp::Ordering::Less)]
    fn test_cmp<T>(
        #[case] input: OneOrMany<T>,
        #[case] other: OneOrMany<T>,
        #[case] expected: std::cmp::Ordering,
    ) where
        T: std::fmt::Debug + Ord,
    {
        let actual = input.cmp(&other);
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<usize>::None)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]))]
    fn test_eq<T>(#[case] input: OneOrMany<T>, #[case] other: OneOrMany<T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        assert_eq!(input, other);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::One(1))]
    #[case::one(OneOrMany::One(1), OneOrMany::Many(vec![1, 2, 3]))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::<usize>::None)]
    fn test_ne<T>(#[case] input: OneOrMany<T>, #[case] other: OneOrMany<T>)
    where
        T: PartialEq + std::fmt::Debug,
    {
        assert_ne!(input, other);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<usize>::None)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]))]
    #[case::many(OneOrMany::Many(vec![1, 1, 2, 3, 2]), OneOrMany::Many(vec![1, 2, 3]))]
    #[case::many(OneOrMany::Many(vec![1, 1, 1]), OneOrMany::One(1))]
    fn test_dedup<T>(#[case] mut input: OneOrMany<T>, #[case] expected: OneOrMany<T>)
    where
        T: Clone + Eq + std::hash::Hash + std::fmt::Debug,
    {
        input.dedup();
        assert_eq!(input, expected);
    }

    #[rstest]
    #[case::none(OneOrMany::<usize>::None, OneOrMany::<usize>::None)]
    #[case::one(OneOrMany::One(1), OneOrMany::One(1))]
    #[case::many(OneOrMany::Many(vec![1, 2, 3]), OneOrMany::Many(vec![1, 2, 3]))]
    #[case::many(OneOrMany::Many(vec![1, 1, 2, 3, 2]), OneOrMany::Many(vec![1, 2, 3]))]
    #[case::many(OneOrMany::Many(vec![1, 1, 1]), OneOrMany::One(1))]
    fn test_dedup_by_key<T>(#[case] mut input: OneOrMany<T>, #[case] expected: OneOrMany<T>)
    where
        T: Clone + Eq + std::hash::Hash + std::fmt::Debug,
    {
        input.dedup_by_key(Clone::clone);
        assert_eq!(input, expected);
    }
}
