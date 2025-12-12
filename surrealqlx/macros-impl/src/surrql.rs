use proc_macro2::TokenStream;
use syn::LitStr;

/// Implementation of the `surrql!` macro that parses a `SurrealQL` query at compile time.
/// # Arguments
/// * `input` - The input token stream containing the `SurrealQL` query as a string literal.
/// # Returns
/// A `TokenStream` containing the original query if parsing is successful, or a
/// `syn::Error` if parsing fails.
/// # Errors
/// Returns a `syn::Error` if the input is not a valid string literal or if
/// the `SurrealQL` query cannot be parsed.
pub fn surrql_macro_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let query = input.clone();
    let parsed = syn::parse2::<LitStr>(input)?;

    match surrealdb::sql::parse(&parsed.value()) {
        Ok(_) => Ok(query),
        Err(err) => Err(syn::Error::new_spanned(query, err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_valid_query() {
        let input = quote! {
            "SELECT * FROM person WHERE age > 30"
        };
        let result = surrql_macro_impl(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_query() {
        let input = quote! {
            "SELEC * FROM person" // typo in SELECT
        };
        let result = surrql_macro_impl(input);
        assert!(result.is_err());
    }
}
