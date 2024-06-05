mod iter;
pub use iter::OneOrManyIter;
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
/// Especially usefull when working with deserialing data
///
/// To let it be useful in other contexts, it aims to implement many of the same traits and functions as `Vec<T>` and `Option<T>`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
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
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(t) => t.len(),
            Self::None => 0,
        }
    }

    /// Returns `true` if the `OneOrMany` is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the value at the given index, or `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<&T> {
        match self {
            Self::One(t) => {
                if index == 0 {
                    Some(t)
                } else {
                    None
                }
            }
            Self::Many(t) => t.get(index),
            Self::None => None,
        }
    }

    /// Returns the first value, or `None` if the `OneOrMany` is empty.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Returns `true` if the `OneOrMany` contains the given value.
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
    pub fn push(&mut self, new: T)
    where
        T: Clone,
    {
        match self {
            Self::One(t) => {
                *self = Self::Many(vec![t.clone(), new]);
            }
            Self::Many(t) => t.push(new),
            Self::None => *self = Self::One(new),
        }
    }

    /// Pops a value from the end of the `OneOrMany`.
    pub fn pop(&mut self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Self::One(t) => {
                let old = t.clone();
                *self = Self::None;
                Some(old)
            }
            Self::Many(t) => t.pop(),
            Self::None => None,
        }
    }

    /// Checks if the `OneOrMany` is `None`.
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Checks if the `OneOrMany` is `One`.
    pub const fn is_one(&self) -> bool {
        matches!(self, Self::One(_))
    }

    /// Checks if the `OneOrMany` is `Many`.
    pub const fn is_many(&self) -> bool {
        matches!(self, Self::Many(_))
    }

    /// Checks if the `OneOrMany` is `One` or `Many`.
    pub const fn is_some(&self) -> bool {
        self.is_one() || self.is_many()
    }

    /// Gets a slice of the `OneOrMany`.
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(t) => std::slice::from_ref(t),
            Self::Many(t) => t,
            Self::None => &[],
        }
    }

    /// Gets a mutable slice of the `OneOrMany`.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Self::One(t) => std::slice::from_mut(t),
            Self::Many(t) => t,
            Self::None => &mut [],
        }
    }
}

impl<T> From<T> for OneOrMany<T> {
    fn from(t: T) -> Self {
        Self::One(t)
    }
}

impl<T> From<Option<T>> for OneOrMany<T> {
    fn from(t: Option<T>) -> Self {
        t.map_or_else(|| Self::None, |t| Self::One(t))
    }
}

impl<T> From<Option<Self>> for OneOrMany<T> {
    fn from(t: Option<Self>) -> Self {
        t.map_or_else(|| Self::None, |t| t)
    }
}

impl<T: Clone> From<&[T]> for OneOrMany<T> {
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

#[allow(clippy::fallible_impl_from)] // we check the length so it's fine
impl<T> From<Vec<T>> for OneOrMany<T> {
    fn from(t: Vec<T>) -> Self {
        if t.is_empty() {
            Self::None
        } else if t.len() == 1 {
            Self::One(t.into_iter().next().unwrap())
        } else {
            Self::Many(t)
        }
    }
}

impl<T> From<OneOrMany<T>> for Vec<T> {
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
impl<T> PartialOrd<OneOrMany<T>> for OneOrMany<T>
where
    T: PartialOrd,
{
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
