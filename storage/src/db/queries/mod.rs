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
#[track_caller]
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

    format!("DEFINE ANALYZER OVERWRITE {name}{tokenizer_string}{filter_string}")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::basic(
        "test",
        Some(Tokenizer::Class),
        vec!["snowball(english)"],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class FILTERS snowball(english);"
    )]
    #[case::no_tokenizers(
        "test",
        None,
        vec!["snowball(english)"],
        "DEFINE ANALYZER OVERWRITE test FILTERS snowball(english);"
    )]
    #[case::no_filters(
        "test",
        Some(Tokenizer::Class),
        vec![],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class;"
    )]
    #[case::no_tokenizers_or_filters("test", None, vec![], "DEFINE ANALYZER  OVERWRITE test;")]
    // #[case::multiple_tokenizers(
    //     "test",
    //     vec![Tokenizer::Class, Tokenizer::Punct],
    //     vec!["snowball(english)"],
    //     "DEFINE ANALYZER OVERWRITE test TOKENIZERS class,simple FILTERS snowball(english);"
    // )]
    #[case::multiple_filters(
        "test",
        Some(Tokenizer::Class),
        vec!["snowball(english)", "lowercase"],
        "DEFINE ANALYZER OVERWRITE test TOKENIZERS class FILTERS snowball(english),lowercase;"
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
        let statements = statement.into_query().unwrap();

        assert_eq!(statements, expected.into_query().unwrap());
        assert!(!statements.is_empty());
    }
}
