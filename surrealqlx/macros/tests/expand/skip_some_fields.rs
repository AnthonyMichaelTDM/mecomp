#[macro_use]
extern crate surrealqlx_macros;

#[derive(Table)]
#[Table("users")]
struct User {
    #[field(skip)]
    id: i32,
    #[field(dt = "string")]
    name: String,
    #[field(dt = "int")]
    age: i32,
    #[field(dt = "array<int>")]
    favorite_numbers: Vec<i32>,
}
