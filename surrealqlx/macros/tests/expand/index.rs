#[macro_use]
extern crate surrealqlx_macros;

#[derive(Table)]
#[Table("users")]
struct User {
    #[field(dt = "string", index(unique))]
    name: String,
    #[field(dt = "int", index)]
    age: i32,
    #[field("int", index)]
    age2: i32,
    #[field(dt = "array<int>", index(vector(dim = 7)))]
    favorite_numbers: [i32; 7],
    #[field(dt = "array<int>", index(unique, vector(7)))]
    favorite_numbers2: [i32; 7],
}
