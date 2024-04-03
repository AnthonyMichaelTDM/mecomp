pub mod traits;
#[cfg(feature = "macros")]
pub use surrealqlx_macros::*;

/// Macro to register a table in the database,
/// syntax:
/// ```ignore
/// register_tables!(
///     DB, // your `Surreal<_>` database connection
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
            ) -> ::surrealdb::Result<()> {
                $(
                    <$table as ::surrealqlx::traits::Table>::init_table(db).await?;
                )*
                Ok(())
            }
            init_($db_conn).await
        }
    };
}
