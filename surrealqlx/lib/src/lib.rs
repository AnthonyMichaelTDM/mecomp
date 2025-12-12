pub mod migrations;
pub mod traits;
#[cfg(feature = "macros")]
#[doc(inline)]
pub use surrealqlx_macros::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SurrealDB error: {0}")]
    SurrealDb(#[from] surrealdb::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] migrations::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Macro to register a table in the database,
/// syntax:
/// ```ignore
/// register_tables!(
///     &db, // your `Surreal<_>` database connection
///     Table1, // your tables...
///     Table2,
///     ...
/// ).await?;
/// ```
#[cfg(feature = "macros")]
#[macro_export]
macro_rules! register_tables {
    ($db_conn: expr, $($table:ty),*) => {
        {
            async fn init_<C: ::surrealdb::Connection>(
                db: &::surrealdb::Surreal<C>,
            ) -> ::surrealqlx::migrations::Result<()> {
                $(
                    <$table as ::surrealqlx::traits::Table>::init_table(db).await?;
                )*
                Ok(())
            }
            init_($db_conn).await
        }
    };
}
