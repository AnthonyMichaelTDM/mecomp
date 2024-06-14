use surrealdb::{
    opt::IntoQuery,
    sql::{statements::DefineStatement, Statement, Tokenizer},
};

pub mod album;
#[cfg(feature = "analysis")]
pub mod analysis;
pub mod artist;
pub mod collection;
pub mod generic;
pub mod playlist;
pub mod song;

// NOTE: blocked on https://github.com/surrealdb/surrealdb/pull/4156,
// when merged, we can uncomment this
// use surrealdb::sql::{
//     statements::{DefineAnalyzerStatement, DefineStatement},
//     Filter, Ident, Tokenizer,
// };
// pub fn define_analyzer(
//     name: &str,
//     tokenizers: Vec<Tokenizer>,
//     filters: Vec<Filter>,
// ) -> DefineStatement {
//     DefineStatement::Analyzer(DefineAnalyzerStatement {
//         name: Ident(name.to_string()),
//         tokenizers: Some(tokenizers),
//         filters: Some(filters),
//         comment: None,
//     })
// }

/// NOTE: for some reason, having more than one tokenizer causes the parser to fail, so we're just not going to support that for now
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn define_analyzer(
    name: &str,
    tokenizer: Option<Tokenizer>,
    filters: &[&str],
) -> DefineStatement {
    // allowed to maintain style (and make it easier to revert to a vec of tokenizers if needed)
    #[allow(clippy::option_if_let_else)]
    let tokenizer_string = if let Some(tokenizer) = tokenizer {
        String::from(" TOKENIZERS ") + &tokenizer.to_string()
    } else {
        String::new()
    };

    let filter_string = if filters.is_empty() {
        String::new()
    } else {
        let filters = filters.join(",");
        String::from(" FILTERS ") + &filters
    };

    match format!("DEFINE ANALYZER {name}{tokenizer_string}{filter_string} ;")
        .into_query()
        .unwrap()
        .first()
        .unwrap()
    {
        Statement::Define(define_statement) => define_statement.clone(),
        _ => unreachable!(),
    }
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
        "DEFINE ANALYZER test TOKENIZERS class FILTERS snowball(english);"
    )]
    #[case::no_tokenizers(
        "test",
        None,
        vec!["snowball(english)"],
        "DEFINE ANALYZER test FILTERS snowball(english);"
    )]
    #[case::no_filters(
        "test",
        Some(Tokenizer::Class),
        vec![],
        "DEFINE ANALYZER test TOKENIZERS class;"
    )]
    #[case::no_tokenizers_or_filters("test", None, vec![], "DEFINE ANALYZER test;")]
    // #[case::multiple_tokenizers(
    //     "test",
    //     vec![Tokenizer::Class, Tokenizer::Punct],
    //     vec!["snowball(english)"],
    //     "DEFINE ANALYZER test TOKENIZERS class,simple FILTERS snowball(english);"
    // )]
    #[case::multiple_filters(
        "test",
        Some(Tokenizer::Class),
        vec!["snowball(english)", "lowercase"],
        "DEFINE ANALYZER test TOKENIZERS class FILTERS snowball(english),lowercase;"
    )]
    // #[case::multiple_tokenizers_and_filters(
    //     "test",
    //     vec![Tokenizer::Class, Tokenizer::Punct],
    //     vec!["snowball(english)", "lowercase"],
    //     "DEFINE ANALYZER test TOKENIZERS class,simple FILTERS snowball(english),lowercase;"
    // )]
    fn test_define_analyzer(
        #[case] name: &str,
        #[case] tokenizer: Option<Tokenizer>,
        #[case] filters: Vec<&str>,
        #[case] expected: &str,
    ) {
        let statement = define_analyzer(name, tokenizer, &filters);

        assert_eq!(
            statement.into_query().unwrap(),
            expected.into_query().unwrap()
        );
    }
}
