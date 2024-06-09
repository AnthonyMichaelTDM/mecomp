use crate::table_macro_impl;
use pretty_assertions::assert_str_eq;
use proc_macro2::TokenStream;
use quote::quote;
use rstest::rstest;

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

    let expanded = table_macro_impl(input).unwrap();
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

    let expanded = table_macro_impl(input).unwrap();
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

    let expanded = table_macro_impl(input).unwrap();
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
            #[field(index(), dt = "int")]
            age: i32,
            #[field("int", index())]
            age2: i32,
            #[field(dt = "array<int>", index(vector(dim = 7)))]
            favorite_numbers: [i32; 7],
            #[field(dt = "array<int>", index(vector(7)))]
            favorite_numbers2: [i32; 7],
            #[field(dt = "string", index(text("analyzer")))]
            text: String,
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
                        .query("DEFINE FIELD text ON users TYPE string;")
                        .query("COMMIT;")
                        .query("BEGIN;")
                        .query("DEFINE INDEX users_name_index ON users FIELDS name UNIQUE;")
                        .query("DEFINE INDEX users_age_index ON users FIELDS age;")
                        .query("DEFINE INDEX users_age2_index ON users FIELDS age2;")
                        .query(
                            "DEFINE INDEX users_favorite_numbers_index ON users FIELDS favorite_numbers MTREE DIMENSION 7;",
                        )
                        .query(
                            "DEFINE INDEX users_favorite_numbers2_index ON users FIELDS favorite_numbers2 MTREE DIMENSION 7;",
                        )
                        .query(
                            "DEFINE INDEX users_text_index ON users FIELDS text SEARCH ANALYZER analyzer BM25;",
                        )
                        .query("COMMIT;")
                        .await?;
                    Ok(())
                }
            }
        }
    };
    let pretty_output = prettyplease::unparse(&syn::parse_file(&output).unwrap());

    let expanded = table_macro_impl(input).unwrap();
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

    let expanded = table_macro_impl(input).unwrap();
    let pretty_expanded = prettyplease::unparse(&syn::parse_file(&expanded.to_string()).unwrap());

    assert_str_eq!(pretty_output, pretty_expanded);
}

#[rstest]
#[case::missing_table_attr(quote!{ struct User { #[field(dt = "string")] name: String, }})]
#[case::missing_table_name(quote!{ #[Table] struct User { #[field(dt = "string")] name: String, }})]
#[case::invalid_table_name(quote!{ #[Table(1)] struct User { #[field(dt = "string")] name: String, }})]
#[case::invalid_table_name2(quote!{ #[Table(foo())] struct User { #[field(dt = "string")] name: String, }})]
fn test_invalid_table_attr(#[case] input: TokenStream) {
    let expanded = table_macro_impl(input);
    assert!(expanded.is_err());
}

#[rstest]
#[case::enum_(quote!{#[Table("users")] enum User { A, B, C, }})]
#[case::trait_(quote!{#[Table("users")] trait User { fn foo(&self); }})]
#[case::impl_(quote!{#[Table("users")] impl User { fn foo(&self) {} }})]
#[case::type_(quote!{#[Table("users")] type User = i32;})]
#[case::unit_struct(quote!{#[Table("users")] struct User(String, i32, Vec<i32>);})]
#[case::empty_struct(quote!{#[Table("users")] struct User{};})]
fn test_fails_for_non_structs(#[case] input: TokenStream) {
    let expanded = table_macro_impl(input);
    assert!(expanded.is_err());
}

#[rstest]
#[case::not_a_call_expr(quote!{ #[Table("users")] struct User { #[field(dt = "string", index)] name: String, }})]
#[case::invalid_arg(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(invalid_option))] name: String, }})]
#[case::unique_not_bool(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(unique = foo))] name: String, }})]
#[case::vector_not_a_call_expr(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector))] name: String, }})]
#[case::vector_invalid_dim(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector()))] name: String, }})]
#[case::vector_invalid_dim(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector(0)))] name: String, }})]
#[case::vector_invalid_dim(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector(dim)))] name: String, }})]
#[case::vector_invalid_dim(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector(dim = "not a number")))] name: String, }})]
#[case::vector_invalid_dim(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector("not a number")))] name: String, }})]
#[case::cant_mix_index_types(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(unique, vector(1)))] name: String, }})]
#[case::cant_mix_index_types(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(unique, text("analyzer")))] name: String, }})]
#[case::cant_mix_index_types(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(vector(1), text("analyzer")))] name: String, }})]
#[case::text_not_a_call_expr(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(text))] name: String, }})]
#[case::text_invalid_analyzer(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(text()))] name: String, }})]
#[case::text_invalid_analyzer(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(text(0)))] name: String, }})]
#[case::text_invalid_analyzer(quote!{ #[Table("users")] struct User { #[field(dt = "string", index(text(analyzer)))] name: String, }})]
fn test_invalid_index(#[case] input: TokenStream) {
    let expanded = table_macro_impl(input);
    assert!(expanded.is_err());
}

#[rstest]
#[case(quote!{ #[Table("users")] struct User { #[field(dt = "string", invalid)] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(dt = "string", invalid(foo))] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(dt = 1)] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field("string" = dt)] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(dt = foo())] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(1)] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(foo - bar)] name: String, }})]
#[case(quote!{ #[Table("users")] struct User { #[field(index())] name: String, }})]
#[case::missing(quote!{ #[Table("users")] struct User { name: String, }})]
#[case::missing_dt(quote!{ #[Table("users")] struct User { #[field] name: String, }})]
fn test_invalid_field(#[case] input: TokenStream) {
    let expanded = table_macro_impl(input);
    assert!(expanded.is_err());
}
