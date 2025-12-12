use surrealdb::{Connection, Surreal};

use crate::migrations::{M, Migrations, Result};

pub trait Table {
    const TABLE_NAME: &'static str;

    fn migrations() -> Vec<M<'static>>;

    #[must_use]
    fn init_table<C: Connection>(
        db: &Surreal<C>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let migrations = Migrations::new(Self::TABLE_NAME, Self::migrations());
        async move { migrations.to_latest(db).await }
    }
}
