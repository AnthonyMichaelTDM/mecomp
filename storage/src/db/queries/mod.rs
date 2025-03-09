use surrealdb::{opt::IntoQuery, sql::Tokenizer};

pub mod album;
#[cfg(feature = "analysis")]
pub mod analysis;
pub mod artist;
pub mod collection;
pub mod generic;
pub mod playlist;
pub mod song;

/// NOTE: for some reason, having more than one tokenizer causes the parser to fail, so we're just not going to support that for now
#[must_use]
#[inline]
pub fn define_analyzer(
    name: &str,
    tokenizer: Option<Tokenizer>,
    filters: &[&str],
) -> impl IntoQuery {
    let tokenizer_string = tokenizer.map_or_else(String::new, |t| format!(" TOKENIZERS {t}"));

    let filter_string = filters.is_empty().then(String::new).unwrap_or_else(|| {
        let filters = filters.join(",");
        format!(" FILTERS {filters}")
    });

    parse_query(format!(
        "DEFINE ANALYZER OVERWRITE {name}{tokenizer_string}{filter_string}"
    ))
}

/// Parse a query (string) into a `surrealdb::sql::Query`
///
/// This is primarily used to validate the syntax of queries before they are executed
pub fn parse_query(query: impl AsRef<str>) -> surrealdb::sql::Query {
    surrealdb::syn::parse(query.as_ref()).unwrap()
}

#[cfg(test)]
pub fn validate_query(query: impl IntoQuery, expected: &str) {
    use pretty_assertions::assert_eq;
    // first check if we can use IntoQuery to parse the query
    let compiled_query: surrealdb::sql::Query = query
        .as_str()
        .map(surrealdb::syn::parse)
        .map_or_else(|| query.into_query().unwrap().into(), Result::unwrap);

    let compiled_expected = surrealdb::syn::parse(expected).unwrap();
    assert!(
        compiled_expected.0.len() > 0,
        "Expected query compiled to an empty list of statements: \"{expected}\""
    );
    assert_eq!(compiled_query, compiled_expected);
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::basic(
        "test",
        Some(Tokenizer::Class),
        vec!["snowball(English)"],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class FILTERS snowball(English);"
    )]
    #[case::no_tokenizers(
        "test",
        None,
        vec!["snowball(English)"],
        "DEFINE ANALYZER OVERWRITE test FILTERS snowball(English);"
    )]
    #[case::no_filters(
        "test",
        Some(Tokenizer::Class),
        vec![],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class;"
    )]
    #[case::no_tokenizers_or_filters("test", None, vec![], "DEFINE ANALYZER OVERWRITE test;")]
    // #[case::multiple_tokenizers(
    //     "test",
    //     vec![Tokenizer::Class, Tokenizer::Punct],
    //     vec!["snowball(english)"],
    //     "DEFINE ANALYZER OVERWRITE test TOKENIZERS class,simple FILTERS snowball(english);"
    // )]
    #[case::multiple_filters(
        "test",
        Some(Tokenizer::Class),
        vec!["snowball(English)", "lowercase"],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class FILTERS snowball(English),lowercase;"
    )]
    // #[case::multiple_tokenizers_and_filters(
    //     "test",
    //     vec![Tokenizer::Class, Tokenizer::Punct],
    //     vec!["snowball(english)", "lowercase"],
    //     "DEFINE ANALYZER OVERWRITE test TOKENIZERS class,simple FILTERS snowball(english),lowercase;"
    // )]
    fn test_define_analyzer(
        #[case] name: &str,
        #[case] tokenizer: Option<Tokenizer>,
        #[case] filters: Vec<&str>,
        #[case] expected: &str,
    ) {
        let statement = define_analyzer(name, tokenizer, &filters);

        validate_query(statement, expected);
    }
}
