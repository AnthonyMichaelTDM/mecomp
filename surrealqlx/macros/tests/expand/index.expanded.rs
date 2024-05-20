#[macro_use]
extern crate surrealqlx_macros;
#[Table("users")]
struct User {
    #[field(dt = "string", index(unique))]
    name: String,
    #[field(dt = "int", index())]
    age: i32,
    #[field("int", index())]
    age2: i32,
    #[field(dt = "array<int>", index(vector(dim = 7)))]
    favorite_numbers: [i32; 7],
    #[field(dt = "array<int>", index(unique, vector(7)))]
    favorite_numbers2: [i32; 7],
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
                .query("DEFINE FIELD age2 ON users TYPE int;")
                .query("DEFINE FIELD favorite_numbers ON users TYPE array<int>;")
                .query("DEFINE FIELD favorite_numbers2 ON users TYPE array<int>;")
                .query("COMMIT;")
                .query("BEGIN;")
                .query("DEFINE INDEX users_name_index ON users FIELDS name UNIQUE;")
                .query("DEFINE INDEX users_age_index ON users FIELDS age;")
                .query("DEFINE INDEX users_age2_index ON users FIELDS age2;")
                .query(
                    "DEFINE INDEX users_favorite_numbers_index ON users FIELDS favorite_numbers MTREE DIMENSION 7;",
                )
                .query(
                    "DEFINE INDEX users_favorite_numbers2_index ON users FIELDS favorite_numbers2 UNIQUE MTREE DIMENSION 7;",
                )
                .query("COMMIT;")
                .await?;
            Ok(())
        }
    }
}
