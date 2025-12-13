use proc_macro::TokenStream;
use syn::parse_macro_input;

use surrealqlx_macros_impl::table_macro_impl;

#[cfg(not(tarpaulin_include))]
#[proc_macro_derive(Table, attributes(Table, field, index))]
pub fn table_macro(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input);
    match table_macro_impl(input) {
        Ok(out) => out.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Macro to parse a `SurrealQL` query at compile time.
///
/// Returns a value that implements `IntoQuery`, which can be passed to `db.query()`.
///
/// # Examples
///
/// ```rust,ignore
/// use surrealqlx::surrql;
/// use surrealdb::Surreal;
/// use surrealdb::engine::any::Any;
///
/// async fn example(db: &Surreal<Any>) -> surrealdb::Result<()> {
///     // Use with db.query()
///     db.query(surrql!("SELECT * FROM person WHERE age > 30;")).await?;
///
///     // Use in a function that returns impl IntoQuery
///     fn get_query() -> impl surrealdb::opt::IntoQuery {
///         surrql!("SELECT marketing, count() FROM type::table($table) GROUP BY marketing;")
///     }
///     Ok(())
/// }
/// ```
#[proc_macro]
pub fn surrql(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    match surrealqlx_macros_impl::surrql_macro_impl(input) {
        Ok(out) => out.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
