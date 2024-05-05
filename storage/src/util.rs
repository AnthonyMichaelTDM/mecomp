//! Utility types and functions.

use std::clone::Clone;

use surrealdb::opt::QueryResult;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
    #[default]
    None,
}

impl<T> OneOrMany<T> {
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(t) => t.len(),
            Self::None => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

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

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub const fn iter(&self) -> OneOrManyIter<T> {
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
            Self::One(t) => t == genre,
            Self::Many(t) => t.contains(genre),
            Self::None => false,
        }
    }

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

    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub const fn is_one(&self) -> bool {
        matches!(self, Self::One(_))
    }

    pub const fn is_many(&self) -> bool {
        matches!(self, Self::Many(_))
    }

    pub const fn is_some(&self) -> bool {
        self.is_one() || self.is_many()
    }

    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(t) => std::slice::from_ref(t),
            Self::Many(t) => t,
            Self::None => &[],
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

impl<T: std::clone::Clone> FromIterator<T> for OneOrMany<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter: <I as IntoIterator>::IntoIter = iter.into_iter();
        let mut result: Self = Self::None;
        for item in iter {
            result.push(item);
        }
        result
    }
}

impl<T> QueryResult<OneOrMany<T>> for usize
where
    T: serde::Serialize + for<'a> serde::Deserialize<'a> + Clone,
{
    /// we can't access the interior `results` field of `response` because it's private, so we can't
    /// implement this trait directly.
    /// Instead, we'll implement use the impl's for `QueryResult` for `Vec<T>` and `Option<T>` to
    /// implement this trait for `OneOrMany<T>`.
    fn query_result(self, response: &mut surrealdb::Response) -> surrealdb::Result<OneOrMany<T>> {
        let vec: surrealdb::Result<Vec<T>> = self.query_result(response);

        if let Ok(vec) = vec {
            Ok(vec.into())
        } else {
            let one: Option<T> = self.query_result(response)?;
            Ok(one.into())
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MetadataConflictResolution {
    Merge,
    Overwrite,
    Skip,
}

#[cfg(test)]
mod test_one_or_many {
    use crate::db::init_test_database;

    use super::OneOrMany;

    use pretty_assertions::assert_eq;
    use serde::{Deserialize, Serialize};
    use surrealdb::sql::{Id, Thing};
    use surrealqlx::{register_tables, Table};

    #[derive(Serialize, Deserialize, Table, PartialEq, Eq, Debug)]
    #[Table("one_or_many_test_table")]
    struct TestStruct {
        #[field("option<array<int> | int>")]
        #[serde(default)]
        foo: OneOrMany<usize>,
    }

    #[tokio::test]
    async fn test_read_write_none() -> anyhow::Result<()> {
        let db = init_test_database().await?;
        register_tables!(&db, TestStruct)?;

        let thing = Thing::from(("one_or_many_test_table", Id::ulid()));

        // store a None varient into the database
        let create: TestStruct = db
            .create(thing.clone())
            .content(TestStruct {
                foo: OneOrMany::None,
            })
            .await?
            .unwrap();

        // read a None variant from the database
        let read: TestStruct = db.select(thing).await?.unwrap();

        assert_eq!(create, read);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_write_one() -> anyhow::Result<()> {
        let db = init_test_database().await?;
        register_tables!(&db, TestStruct)?;

        let thing = Thing::from(("one_or_many_test_table", Id::ulid()));

        // store a None varient into the database
        let create: TestStruct = db
            .create(thing.clone())
            .content(TestStruct {
                foo: OneOrMany::One(3),
            })
            .await?
            .unwrap();

        // read a None variant from the database
        let read: TestStruct = db.select(thing).await?.unwrap();

        assert_eq!(create, read);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_write_many() -> anyhow::Result<()> {
        let db = init_test_database().await?;
        register_tables!(&db, TestStruct)?;

        let thing = Thing::from(("one_or_many_test_table", Id::ulid()));

        // store a None varient into the database
        let create: TestStruct = db
            .create(thing.clone())
            .content(TestStruct {
                foo: OneOrMany::Many(vec![1, 2, 3]),
            })
            .await?
            .unwrap();

        // read a None variant from the database
        let read: TestStruct = db.select(thing).await?.unwrap();

        assert_eq!(create, read);

        Ok(())
    }
}
