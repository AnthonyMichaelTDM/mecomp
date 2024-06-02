use proc_macro::TokenStream;
use syn::parse_macro_input;

use surrealqlx_macros_impl::table_macro_impl;

#[proc_macro_derive(Table, attributes(Table, field))]
pub fn table_macro(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input);
    table_macro_impl(input).into()
}
