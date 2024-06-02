use crate::table_macro_impl;
use pretty_assertions::assert_str_eq;
use quote::quote;

#[test]
fn test_album() {
    let input = quote! {
        #[derive(Table)]
        #[Table("album")]
        pub struct Album {
            #[field(dt = "record")]
            pub id: AlbumId,
            #[field(dt = "string", index())]
            pub title: Arc<str>,
            #[field(dt = "set<record> | record")]
            pub artist_id: OneOrMany<ArtistId>,
            #[field(dt = "set<string> | string")]
            pub artist: OneOrMany<Arc<str>>,
            #[field(dt = "option<int>")]
            pub release: Option<i32>,
            #[field(dt = "duration")]
            pub runtime: Duration,
            #[field(dt = "int")]
            pub song_count: usize,
            #[field(dt = "set<record>")]
            pub songs: Box<[SongId]>,
            #[field(dt = "int")]
            pub discs: u32,
            #[field(dt = "set<string> | string")]
            pub genre: OneOrMany<Arc<str>>,
        }
    };

    let output = stringify! {
        impl ::surrealqlx::traits::Table for Album {
            const TABLE_NAME: &'static str = "album";
            #[allow(manual_async_fn)]
            fn init_table<C: ::surrealdb::Connection>(
                db: &::surrealdb::Surreal<C>,
            ) -> impl ::std::future::Future<Output = ::surrealdb::Result<()>> + Send {
                async {
                    let _ = db
                        .query("BEGIN;")
                        .query("DEFINE TABLE album SCHEMAFULL;")
                        .query("COMMIT;")
                        .query("BEGIN;")
                        .query("DEFINE FIELD id ON album TYPE record;")
                        .query("DEFINE FIELD title ON album TYPE string;")
                        .query("DEFINE FIELD artist_id ON album TYPE set<record> | record;")
                        .query("DEFINE FIELD artist ON album TYPE set<string> | string;")
                        .query("DEFINE FIELD release ON album TYPE option<int>;")
                        .query("DEFINE FIELD runtime ON album TYPE duration;")
                        .query("DEFINE FIELD song_count ON album TYPE int;")
                        .query("DEFINE FIELD songs ON album TYPE set<record>;")
                        .query("DEFINE FIELD discs ON album TYPE int;")
                        .query("DEFINE FIELD genre ON album TYPE set<string> | string;")
                        .query("COMMIT;")
                        .query("BEGIN;")
                        .query("DEFINE INDEX album_title_index ON album FIELDS title;")
                        .query("COMMIT;")
                        .await?;
                    Ok(())
                }
            }
        }
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input);
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}

#[test]
fn test_bare_type() {
    let input = quote! {
        #[Table("users")]
        struct User {
            #[field("string")]
            name: String,
            #[field("int")]
            age: i32,
            #[field("array<int>")]
            favorite_numbers: Vec<i32>,
        }
    };

    let output = stringify! {
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
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input);
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}

#[test]
fn test_basic() {
    let input = quote! {
        #[Table("users")]
        struct User {
            #[field(dt = "string")]
            name: String,
            #[field(dt = "int")]
            age: i32,
            #[field(dt = "array<int>")]
            favorite_numbers: Vec<i32>,
        }
    };

    let output = stringify! {
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
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input);
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}

#[test]
fn test_index() {
    let input = quote! {
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
    };

    let output = stringify! {
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
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input);
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}

#[test]
fn test_skip_some_fields() {
    let input = quote! {
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
    };

    let output = stringify! {
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
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input);
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}
