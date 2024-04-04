#[macro_use]
extern crate surrealqlx_macros;
#[Table("users")]
struct User {
    #[field(dt = "string")]
    name: String,
    #[field(dt = "int")]
    age: i32,
    #[field(dt = "array<int>")]
    favorite_numbers: Vec<i32>,
}
impl ::surrealqlx::traits::Table for User {
    const TABLE_NAME: &'static str = "users";
    const TABLE_SCHEMA_QUERY: &'static str = "BEGIN TRANSACTION;\n DEFINE TABLE IF NOT EXISTS users SCHEMAFULL;\nDEFINE FIELD IF NOT EXISTS name ON users TYPE string;\nDEFINE FIELD IF NOT EXISTS age ON users TYPE int;\nDEFINE FIELD IF NOT EXISTS favorite_numbers ON users TYPE array<int>;\n\nCOMMIT TRANSACTION;";
}
