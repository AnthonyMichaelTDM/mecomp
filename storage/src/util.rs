//! Utility types and functions.

use std::clone::Clone;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
    None,
}

impl<T> Default for OneOrMany<T> {
    fn default() -> Self {
        OneOrMany::None
    }
}

impl<T> OneOrMany<T> {
    pub fn len(&self) -> usize {
        match self {
            OneOrMany::One(_) => 1,
            OneOrMany::Many(t) => t.len(),
            OneOrMany::None => 0,
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        match self {
            OneOrMany::One(t) => {
                if index == 0 {
                    Some(t)
                } else {
                    None
                }
            }
            OneOrMany::Many(t) => t.get(index),
            OneOrMany::None => None,
        }
    }

    pub fn iter(&self) -> OneOrManyIter<T> {
        OneOrManyIter {
            inner: self,
            index: 0,
        }
    }

    pub fn contains(&self, genre: &T) -> bool
    where
        T: PartialEq,
    {
        match self {
            OneOrMany::One(t) => t == genre,
            OneOrMany::Many(t) => t.contains(genre),
            OneOrMany::None => false,
        }
    }

    pub fn push(&mut self, new: T)
    where
        T: Clone,
    {
        match self {
            OneOrMany::One(t) => {
                *self = OneOrMany::Many(vec![t.clone(), new]);
            }
            OneOrMany::Many(t) => t.push(new),
            OneOrMany::None => *self = OneOrMany::One(new),
        }
    }

    pub fn pop(&mut self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            OneOrMany::One(t) => {
                let old = t.clone();
                *self = OneOrMany::None;
                Some(old)
            }
            OneOrMany::Many(t) => t.pop(),
            OneOrMany::None => None,
        }
    }
}

pub struct OneOrManyIter<'a, T> {
    inner: &'a OneOrMany<T>,
    index: usize,
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

impl<T> From<T> for OneOrMany<T> {
    fn from(t: T) -> Self {
        OneOrMany::One(t)
    }
}

impl<T: Clone> From<&[T]> for OneOrMany<T> {
    fn from(t: &[T]) -> Self {
        if t.len() == 1 {
            OneOrMany::One(t[0].clone())
        } else {
            OneOrMany::Many(t.into())
        }
    }
}

impl<T: Clone> From<Vec<T>> for OneOrMany<T> {
    fn from(t: Vec<T>) -> Self {
        if t.len() == 1 {
            OneOrMany::One(t[0].clone())
        } else {
            OneOrMany::Many(t)
        }
    }
}
