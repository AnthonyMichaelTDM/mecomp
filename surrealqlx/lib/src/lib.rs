pub mod traits;
#[cfg(feature = "macros")]
pub use surrealqlx_macros::*;

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
