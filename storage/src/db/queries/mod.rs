pub mod album;
#[cfg(feature = "analysis")]
pub mod analysis;
pub mod artist;
pub mod collection;
pub mod generic;
pub mod playlist;
pub mod relations;
pub mod song;

/// Parse a query (string) into a `surrealdb::sql::Query`
///
/// This is primarily used to validate the syntax of queries before they are executed
///
/// # Panics
///
/// This function will panic if the query cannot be parsed, which should never happen.
pub fn parse_query(query: impl AsRef<str>) -> surrealdb::sql::Query {
    surrealdb::syn::parse(query.as_ref()).unwrap()
}

#[cfg(test)]
pub fn validate_query(query: impl surrealdb::opt::IntoQuery, expected: &str) {
    use pretty_assertions::assert_eq;
    // first check if we can use IntoQuery to parse the query
    #[expect(deprecated)]
    let compiled_query: surrealdb::sql::Query = query
        .as_str()
        .map(surrealdb::syn::parse)
        .map_or_else(|| query.into_query().unwrap().into(), Result::unwrap);

    let compiled_expected = surrealdb::syn::parse(expected).unwrap();
    assert!(
        !compiled_expected.0.is_empty(),
        "Expected query compiled to an empty list of statements: \"{expected}\""
    );
    assert_eq!(compiled_query, compiled_expected);
}
