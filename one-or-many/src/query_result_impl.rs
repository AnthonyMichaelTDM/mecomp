//! trait impls that let `OneOrMany<T>` be used as a return type from SurrealDB queries.
//!

use surrealdb::opt::QueryResult;

use crate::OneOrMany;

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

#[cfg(test)]
mod tests {

    use crate::OneOrMany;

    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use surrealdb::{
        engine::local::{Db, Mem},
        sql::{Id, Thing},
        Surreal,
    };
    use surrealqlx::{register_tables, Table};

    const TABLE_NAME: &str = "one_or_many_test_table";

    #[derive(Clone, Serialize, Deserialize, Table, PartialEq, Eq, Debug)]
    #[Table("one_or_many_test_table")]
    struct TestStruct {
        #[field("record")]
        id: Thing,
        #[field("option<array<int> | int>")]
        #[serde(default)]
        foo: OneOrMany<usize>,
    }

    impl TestStruct {
        pub fn new(foo: OneOrMany<usize>) -> Self {
            Self {
                id: Thing::from((TABLE_NAME, Id::ulid())),
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
    #[case::one(TestStruct::new(OneOrMany::One(3)))]
    #[case::many(TestStruct::new(OneOrMany::Many(vec![1, 2, 3])))]
    #[tokio::test]
    async fn test_read_write(#[case] to_write: TestStruct) -> anyhow::Result<()> {
        let db = init_test_database().await?;

        // store a None variant into the database
        let create: TestStruct = db
            .create((TABLE_NAME, to_write.id.clone()))
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
    async fn test_oneormany_as_query_result() -> anyhow::Result<()> {
        let db = init_test_database().await?;

        async fn all_items(db: &Surreal<Db>) -> anyhow::Result<OneOrMany<TestStruct>> {
            Ok(db
                .query(format!("SELECT * FROM {TABLE_NAME}"))
                .await?
                .take(0)?)
        }

        // first, read a query that returns nothing
        assert_eq!(all_items(&db).await?, OneOrMany::None);

        // next, we add an item to the database so our next query will return One
        let struct1: TestStruct = TestStruct::new(OneOrMany::One(3));
        let struct1: TestStruct = db
            .create(struct1.id.clone())
            .content(struct1.clone())
            .await?
            .unwrap();

        // read a query that returns one item
        assert_eq!(all_items(&db).await?, OneOrMany::One(struct1.clone()));

        // next, we add another item to the database so our next query will return Many
        let struct2: TestStruct = TestStruct::new(OneOrMany::Many(vec![1, 2, 3]));
        let struct2: TestStruct = db
            .create(struct2.id.clone())
            .content(struct2.clone())
            .await?
            .unwrap();

        // read a query that returns many items
        assert_eq!(
            all_items(&db).await?,
            OneOrMany::Many(vec![struct1, struct2])
        );

        Ok(())
    }
}
