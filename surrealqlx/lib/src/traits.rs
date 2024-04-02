use surrealdb::{Connection, Result, Surreal};

pub trait Table {
    const TABLE_NAME: &'static str;
    const TABLE_SCHEMA_QUERY: &'static str;

    fn init_table<C: Connection>(
        db: &Surreal<C>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async {
            let _: _ = db.query(Self::TABLE_SCHEMA_QUERY).await?;
            Ok(())
        }
    }
}
