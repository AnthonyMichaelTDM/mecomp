//! Module for the query that generates the list of songs for a dynamic playlist.
//!

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The query that generates the list of songs for a dynamic playlist.
pub struct Query {
    pub root: Clause,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A clause in a query.
/// A query is a tree of clauses.
pub enum Clause {
    Compound(CompoundClause),
    Leaf(LeafClause),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A compound clause that is either an OR or an AND.
/// An OR clause is a disjunction of clauses.
/// An AND clause is a conjunction of clauses.
pub struct CompoundClause {
    pub clauses: Vec<Clause>,
    pub kind: CompoundKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The kind of a compound clause.
pub enum CompoundKind {
    Or,
    And,
}

impl CompoundKind {
    #[must_use]
    pub const fn operator(&self) -> &'static str {
        match self {
            Self::Or => " OR ",
            Self::And => " AND ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A single clause in a query.
pub struct LeafClause {
    pub left: Value,
    pub operator: Operator,
    pub right: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The types of values that can be used in a clause.
pub enum Value {
    String(String),
    Int(i64),
    Set(Vec<Value>),
    Field(Field),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
///  The fields of a song that are available for filtering.
pub enum Field {
    // Song
    Title,
    Artists,
    Album,
    AlbumArtists,
    Genre,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The operators that can be used in a clause.
pub enum Operator {
    // Comparison
    Equal,
    NotEqual,
    AnyEqual,
    AllEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    // Fuzzy string comparison
    Like,
    NotLike,
    AnyLike,
    AllLike,
    // Set comparison
    In,
    NotIn,
    Contains,
    ContainsNot,
    ContainsAll,
    ContainsAny,
    ContainsNone,
}

pub trait Compile {
    fn compile(&self) -> String;
}

macro_rules! impl_display {
    ($($t:ty),*) => {
        $(
            impl std::fmt::Display for $t {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.compile())
                }
            }
        )*
    };
}
impl_display!(
    Query,
    Clause,
    CompoundClause,
    LeafClause,
    Value,
    Field,
    Operator
);

impl Compile for Query {
    fn compile(&self) -> String {
        self.root.compile()
    }
}

impl Compile for Clause {
    fn compile(&self) -> String {
        match self {
            Self::Compound(compound) => compound.compile(),
            Self::Leaf(leaf) => leaf.compile(),
        }
    }
}

impl Compile for CompoundClause {
    fn compile(&self) -> String {
        debug_assert!(!self.clauses.is_empty());
        debug_assert_eq!(self.clauses.len(), 2);

        let operator = self.kind.operator();
        let mut clauses = self
            .clauses
            .iter()
            .map(Compile::compile)
            .collect::<Vec<_>>()
            .join(operator);
        if self.clauses.len() > 1 {
            clauses = format!("({clauses})");
        }
        clauses
    }
}

impl Compile for LeafClause {
    fn compile(&self) -> String {
        format!(
            "{} {} {}",
            self.left.compile(),
            self.operator.compile(),
            self.right.compile()
        )
    }
}

impl Compile for Value {
    fn compile(&self) -> String {
        match self {
            Self::String(s) => format!("\"{s}\""),
            Self::Int(i) => i.to_string(),
            Self::Set(set) => {
                let set = set
                    .iter()
                    .map(Compile::compile)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{set}]")
            }
            Self::Field(field) => field.compile(),
        }
    }
}

impl Compile for Field {
    fn compile(&self) -> String {
        match self {
            Self::Title => "title".to_string(),
            Self::Artists => "artist".to_string(),
            Self::Album => "album".to_string(),
            Self::AlbumArtists => "album_artist".to_string(),
            Self::Genre => "genre".to_string(),
            Self::Year => "year".to_string(),
        }
    }
}

impl Compile for Operator {
    fn compile(&self) -> String {
        match self {
            Self::Equal => "=".to_string(),
            Self::NotEqual => "!=".to_string(),
            Self::AnyEqual => "?=".to_string(),
            Self::AllEqual => "*=".to_string(),
            Self::GreaterThan => ">".to_string(),
            Self::GreaterThanOrEqual => ">=".to_string(),
            Self::LessThan => "<".to_string(),
            Self::LessThanOrEqual => "<=".to_string(),
            Self::Like => "~".to_string(),
            Self::NotLike => "!~".to_string(),
            Self::AnyLike => "?~".to_string(),
            Self::AllLike => "*~".to_string(),
            Self::In => "IN".to_string(),
            Self::NotIn => "NOT IN".to_string(),
            Self::Contains => "CONTAINS".to_string(),
            Self::ContainsNot => "CONTAINS NOT".to_string(),
            Self::ContainsAll => "CONTAINS ALL".to_string(),
            Self::ContainsAny => "CONTAINS ANY".to_string(),
            Self::ContainsNone => "CONTAINS NONE".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use rstest_reuse::{apply, template};

    #[template]
    #[rstest]
    #[case::operator(Operator::Equal, "=")]
    #[case::operator(Operator::NotEqual, "!=")]
    #[case::operator(Operator::AnyEqual, "?=")]
    #[case::operator(Operator::AllEqual, "*=")]
    #[case::operator(Operator::GreaterThan, ">")]
    #[case::operator(Operator::GreaterThanOrEqual, ">=")]
    #[case::operator(Operator::LessThan, "<")]
    #[case::operator(Operator::LessThanOrEqual, "<=")]
    #[case::operator(Operator::Like, "~")]
    #[case::operator(Operator::NotLike, "!~")]
    #[case::operator(Operator::AnyLike, "?~")]
    #[case::operator(Operator::AllLike, "*~")]
    #[case::operator(Operator::In, "IN")]
    #[case::operator(Operator::NotIn, "NOT IN")]
    #[case::operator(Operator::Contains, "CONTAINS")]
    #[case::operator(Operator::ContainsNot, "CONTAINS NOT")]
    #[case::operator(Operator::ContainsAll, "CONTAINS ALL")]
    #[case::operator(Operator::ContainsAny, "CONTAINS ANY")]
    #[case::operator(Operator::ContainsNone, "CONTAINS NONE")]
    #[case::field(Field::Title, "title")]
    #[case::field(Field::Artists, "artist")]
    #[case::field(Field::Album, "album")]
    #[case::field(Field::AlbumArtists, "album_artist")]
    #[case::field(Field::Genre, "genre")]
    #[case::field(Field::Year, "year")]
    #[case::value(Value::String("foo".to_string()), "\"foo\"")]
    #[case::value(Value::Int(42), "42")]
    #[case::value(Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)]), "[\"foo\", 42]")]
    #[case::value(Value::Field(Field::Title), "title")]
    #[case::leaf_clause(
        LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::String("foo".to_string())
        },
        "title = \"foo\""
    )]
    #[case::leaf_clause(
        LeafClause {
            left: Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)]),
            operator: Operator::Contains,
            right: Value::Int(42)
        },
        "[\"foo\", 42] CONTAINS 42"
    )]
    #[case::compound_clause(
        CompoundClause {
            clauses: vec![
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Title),
                    operator: Operator::Equal,
                    right: Value::String("foo".to_string())
                }),
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Artists),
                    operator: Operator::Equal,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::And
        },
        "(title = \"foo\" AND artist = \"bar\")"
    )]
    #[case::compound_clause(
        CompoundClause {
            clauses: vec![
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Title),
                    operator: Operator::Equal,
                    right: Value::String("foo".to_string())
                }),
                Clause::Leaf(LeafClause {
                    left: Value::Field(Field::Artists),
                    operator: Operator::Equal,
                    right: Value::String("bar".to_string())
                }),
            ],
            kind: CompoundKind::Or
        },
        "(title = \"foo\" OR artist = \"bar\")"
    )]
    #[case::query(
        Query {
            root: Clause::Compound(CompoundClause {
                clauses: vec![
                    Clause::Compound(
                        CompoundClause {
                            clauses: vec![
                                Clause::Leaf(LeafClause {
                                    left: Value::Field(Field::Title),
                                    operator: Operator::Equal,
                                    right: Value::String("foo".to_string())
                                }),
                                Clause::Compound(CompoundClause {
                                    clauses: vec![
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::Artists),
                                            operator: Operator::Equal,
                                            right: Value::String("bar".to_string())
                                        }),
                                        Clause::Leaf(LeafClause {
                                            left: Value::Field(Field::Album),
                                            operator: Operator::Equal,
                                            right: Value::String("baz".to_string())
                                        }),
                                    ],
                                    kind: CompoundKind::Or
                                }),
                            ],
                            kind: CompoundKind::And
                        }
                    ),
                    Clause::Leaf(LeafClause {
                        left: Value::Field(Field::Year),
                        operator: Operator::GreaterThan,
                        right: Value::Int(2020)
                    }),
                ],
                kind: CompoundKind::And
            })
        },
        "((title = \"foo\" AND (artist = \"bar\" OR album = \"baz\")) AND year > 2020)"
    )]
    fn compilables<T: Compile>(#[case] input: T, #[case] expected: &str) {}

    #[apply(compilables)]
    fn test_compile<T: Compile>(#[case] input: T, #[case] expected: &str) {
        let compiled = input.compile();
        assert_eq!(compiled, expected);
    }

    #[apply(compilables)]
    fn test_display<T: Compile + std::fmt::Display>(#[case] input: T, #[case] expected: &str) {
        let displayed = format!("{}", input);
        assert_eq!(displayed, expected);
    }
}
