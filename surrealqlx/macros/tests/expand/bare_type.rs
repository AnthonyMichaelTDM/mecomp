#[macro_use]
extern crate surrealqlx_macros;

#[derive(Table)]
#[Table("users")]
struct User {
    #[field("string")]
    name: String,
    #[field("int")]
    age: i32,
    #[field("array<int>")]
    favorite_numbers: Vec<i32>,
}
