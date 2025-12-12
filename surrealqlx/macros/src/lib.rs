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
/// # Examples
///
/// ```rust
/// use surrealqlx::surrql;
///
/// surrql!("SELECT * FROM person WHERE age > 30;");
/// surrql!("SELECT marketing, count() FROM type::table($table) GROUP BY marketing;");
/// ```
#[proc_macro]
pub fn surrql(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    match surrealqlx_macros_impl::surrql_macro_impl(input) {
        Ok(out) => out.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
