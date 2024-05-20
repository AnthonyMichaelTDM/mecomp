#[macro_use]
extern crate surrealqlx_macros;
#[Table("users")]
struct User {
    #[field("string")]
    name: String,
    #[field("int")]
    age: i32,
    #[field("array<int>")]
    favorite_numbers: Vec<i32>,
}
impl ::surrealqlx::traits::Table for User {
    const TABLE_NAME: &'static str = "users";
    #[allow(manual_async_fn)]
    fn init_table<C: ::surrealdb::Connection>(
        db: &::surrealdb::Surreal<C>,
    ) -> impl ::std::future::Future<Output = ::surrealdb::Result<()>> + Send {
        async {
            let _ = db
                .query("BEGIN;")
                .query("DEFINE TABLE users SCHEMAFULL;")
                .query("COMMIT;")
                .query("BEGIN;")
                .query("DEFINE FIELD name ON users TYPE string;")
                .query("DEFINE FIELD age ON users TYPE int;")
                .query("DEFINE FIELD favorite_numbers ON users TYPE array<int>;")
                .query("COMMIT;")
                .query("BEGIN;")
                .query("COMMIT;")
                .await?;
            Ok(())
        }
    }
}
