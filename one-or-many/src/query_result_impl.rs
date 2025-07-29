//! trait impls that let `OneOrMany<T>` be used as a return type from `SurrealDB` queries.
//!

use surrealdb::opt::QueryResult;

use crate::OneOrMany;

impl<T> QueryResult<OneOrMany<T>> for usize
where
    T: serde::Serialize + for<'a> serde::Deserialize<'a> + Clone,
{
    /// we can't access the interior `results` field of `response` because it's private, so we can't
    /// implement this trait directly.
    /// Instead, we'll use the impl's for `QueryResult` for `Vec<T>` to
    /// implement this trait for `OneOrMany<T>`.
    #[inline]
    fn query_result(self, response: &mut surrealdb::Response) -> surrealdb::Result<OneOrMany<T>> {
        let vec: Vec<T> = self.query_result(response)?;

        Ok(vec.into())
    }
}

#[cfg(test)]
mod tests {

    use crate::OneOrMany;

    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use surrealdb::{
        RecordId, RecordIdKey, Surreal,
        engine::local::{Db, Mem},
        sql::Id,
    };
    use surrealqlx::{Table, register_tables};

    const TABLE_NAME: &str = "one_or_many_test_table";

    #[derive(Clone, Serialize, Deserialize, Table, PartialEq, Debug)]
    #[Table("one_or_many_test_table")]
    struct TestStruct {
        #[field(dt = "record")]
        id: RecordId,
        #[field(dt = "option<array<int> | int>")]
        #[serde(default)]
        foo: OneOrMany<usize>,
    }

    impl TestStruct {
        pub fn new(foo: OneOrMany<usize>) -> Self {
            Self {
                id: RecordId::from_table_key(TABLE_NAME, RecordIdKey::from_inner(Id::ulid())),
                foo,
            }
        }
    }

    /// Initialize a test database with the same tables as the main database.
    /// This is useful for testing queries and mutations.
    ///
    /// # Errors
    ///
    /// This function will return an error if the database cannot be initialized.
    async fn init_test_database() -> surrealdb::Result<Surreal<Db>> {
        let db = Surreal::new::<Mem>(()).await?;
        db.use_ns("test").use_db("test").await?;

        register_tables!(&db, TestStruct)?;

        Ok(db)
    }

    #[rstest]
    #[case::none(TestStruct::new(OneOrMany::None))]
    #[case::one(TestStruct::new(OneOrMany::from(3)))]
    #[case::many(TestStruct::new(OneOrMany::Many(vec![1, 2, 3])))]
    #[tokio::test]
    async fn test_read_write(#[case] to_write: TestStruct) -> anyhow::Result<()> {
        let db = init_test_database().await?;

        // store a None variant into the database
        let create: TestStruct = db
            .create(to_write.id.clone())
            .content(to_write.clone())
            .await?
            .unwrap();

        // read a None variant from the database
        let read: TestStruct = db.select(to_write.id.clone()).await?.unwrap();

        assert_eq!(create, read);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_as_query_result() -> anyhow::Result<()> {
        async fn all_items(db: &Surreal<Db>) -> anyhow::Result<OneOrMany<TestStruct>> {
            Ok(db
                .query(format!("SELECT * FROM {TABLE_NAME}"))
                .await?
                .take(0)?)
        }

        let db = init_test_database().await?;
        // first, read a query that returns nothing
        assert_eq!(all_items(&db).await?, OneOrMany::None);

        // next, we add an item to the database so our next query will return One
        let struct1: TestStruct = TestStruct::new(OneOrMany::from(3));
        let struct1: TestStruct = db
            .create(struct1.id.clone())
            .content(struct1.clone())
            .await?
            .unwrap();

        // read a query that returns one item
        let result: OneOrMany<TestStruct> = db
            .query(format!("SELECT * FROM {TABLE_NAME} LIMIT 1"))
            .await?
            .take(0)?;
        assert_eq!(result, OneOrMany::from(struct1.clone()));

        // next, we add another item to the database so our next query will return Many
        let struct2: TestStruct = TestStruct::new(OneOrMany::Many(vec![1, 2, 3]));
        let struct2: TestStruct = db
            .create(struct2.id.clone())
            .content(struct2.clone())
            .await?
            .unwrap();

        // read a query that returns many items
        let result = all_items(&db).await?;
        assert!(result.is_many());
        assert_eq!(result.len(), 2);
        assert!(result.contains(&struct1));
        assert!(result.contains(&struct2));

        Ok(())
    }
}
