use surrealdb::{Connection, Result, Surreal};

pub trait Table {
    const TABLE_NAME: &'static str;

    fn init_table<C: Connection>(
        db: &Surreal<C>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}
