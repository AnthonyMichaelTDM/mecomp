//! Module for the query that generates the list of songs for a dynamic playlist.
//!
//! # BNF Grammar
//!
//! ```bnf
//! <query> ::= <clause>
//!
//! <clause> ::= <compound> | <leaf>
//!
//! <compound> ::= (<clause> (" OR " | " AND ") <clause>)
//!
//! <leaf> ::= <value> <operator> <value>
//!
//! <value> ::= <string> | <int> | <set> | <field>
//!
//! <field> ::= "title" | "artist" | "album" | "album_artist" | "genre" | "year"
//!
//! <operator> ::= "=" | "!=" | "?=" | "*=" | ">" | ">=" | "<" | "<=" | "~" | "!~" | "?~" | "*~" | "IN" | "NOT IN" | "CONTAINS" | "CONTAINSNOT" | "CONTAINSALL" | "CONTAINSANY" | "CONTAINSNONE"
//!
//! <string> ::= <quote> { <char> } <quote>
//!
//! <set> ::= '[' <value> { ", " <value> } ']' | '[' ']'
//!
//! <quote> ::= '"' | "'"
//!
//! <int> ::= <digit> { <digit> }
//! ```
//!
//! We will use this grammar as a reference to implement the parser, which we will do using the `pom` crate.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The query that generates the list of songs for a dynamic playlist.
pub struct Query {
    pub root: Clause,
}

impl Serialize for Query {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.compile())
    }
}

impl<'de> Deserialize<'de> for Query {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let query = String::deserialize(deserializer)?;
        Self::from_str(&query).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A clause in a query.
/// A query is a tree of clauses.
pub enum Clause {
    Compound(CompoundClause),
    Leaf(LeafClause),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A compound clause that is either an OR or an AND.
/// An OR clause is a disjunction of clauses.
/// An AND clause is a conjunction of clauses.
pub struct CompoundClause {
    pub clauses: Vec<Clause>,
    pub kind: CompoundKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
/// A single clause in a query.
pub struct LeafClause {
    pub left: Value,
    pub operator: Operator,
    pub right: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The types of values that can be used in a clause.
pub enum Value {
    String(String),
    Int(i64),
    Set(Vec<Value>),
    Field(Field),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Inside,
    NotInside,
    AllInside,
    AnyInside,
    NoneInside,
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
            Self::ContainsNot => "CONTAINSNOT".to_string(),
            Self::ContainsAll => "CONTAINSALL".to_string(),
            Self::ContainsAny => "CONTAINSANY".to_string(),
            Self::ContainsNone => "CONTAINSNONE".to_string(),
            Self::Inside => "INSIDE".to_string(),
            Self::NotInside => "NOTINSIDE".to_string(),
            Self::AllInside => "ALLINSIDE".to_string(),
            Self::AnyInside => "ANYINSIDE".to_string(),
            Self::NoneInside => "NONEINSIDE".to_string(),
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
    #[case::operator(Operator::ContainsNot, "CONTAINSNOT")]
    #[case::operator(Operator::ContainsAll, "CONTAINSALL")]
    #[case::operator(Operator::ContainsAny, "CONTAINSANY")]
    #[case::operator(Operator::ContainsNone, "CONTAINSNONE")]
    #[case::operator(Operator::Inside, "INSIDE")]
    #[case::operator(Operator::NotInside, "NOTINSIDE")]
    #[case::operator(Operator::AllInside, "ALLINSIDE")]
    #[case::operator(Operator::AnyInside, "ANYINSIDE")]
    #[case::operator(Operator::NoneInside, "NONEINSIDE")]
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

    #[apply(compilables)]
    fn test_from_str<T: Compile + std::str::FromStr + std::cmp::PartialEq + std::fmt::Debug>(
        #[case] expected: T,
        #[case] input: &str,
    ) where
        <T as std::str::FromStr>::Err: std::fmt::Debug,
        <T as std::str::FromStr>::Err: PartialEq,
    {
        let parsed = T::from_str(input);
        assert_eq!(parsed, Ok(expected));
    }
}

macro_rules! impl_from_str {
    ($(($t:ty, $p:expr)),*) => {
        $(
            impl std::str::FromStr for $t {
                type Err = pom::Error;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    $p.parse(s.as_bytes())
                }
            }
        )*
    };
}

impl_from_str!(
    (Operator, parser::operator()),
    (Field, parser::field()),
    (Value, parser::value()),
    (LeafClause, parser::leaf()),
    (CompoundClause, parser::compound()),
    (Clause, parser::clause()),
    (Query, parser::query())
);

mod parser {
    use std::str::FromStr;

    use pom::parser::{call, list, none_of, one_of, seq, sym, Parser};

    use super::{Clause, CompoundClause, CompoundKind, Field, LeafClause, Operator, Query, Value};

    pub fn query<'a>() -> Parser<'a, u8, Query> {
        clause().map(|root| Query { root })
    }

    pub fn clause<'a>() -> Parser<'a, u8, Clause> {
        compound().map(Clause::Compound) | leaf().map(Clause::Leaf)
    }

    pub fn compound<'a>() -> Parser<'a, u8, CompoundClause> {
        sym(b'(')
            * space()
            * (call(clause) - space() + (seq(b"AND") | seq(b"OR")) - space() + call(clause)).map(
                |((left, sep), right)| CompoundClause {
                    clauses: vec![left, right],
                    kind: match sep {
                        b"AND" => CompoundKind::And,
                        b"OR" => CompoundKind::Or,
                        _ => unreachable!(),
                    },
                },
            )
            - space()
            - sym(b')')
    }

    pub fn leaf<'a>() -> Parser<'a, u8, LeafClause> {
        (value() - space() + operator() - space() + value()).map(|((left, operator), right)| {
            LeafClause {
                left,
                operator,
                right,
            }
        })
    }

    pub fn value<'a>() -> Parser<'a, u8, Value> {
        string().map(Value::String)
            | int().map(Value::Int)
            | set().map(Value::Set)
            | field().map(Value::Field)
    }

    pub fn field<'a>() -> Parser<'a, u8, Field> {
        seq(b"title").map(|_| Field::Title)
            | seq(b"artist").map(|_| Field::Artists)
            | seq(b"album_artist").map(|_| Field::AlbumArtists)
            | seq(b"album").map(|_| Field::Album)
            | seq(b"genre").map(|_| Field::Genre)
            | seq(b"year").map(|_| Field::Year)
    }

    pub fn operator<'a>() -> Parser<'a, u8, Operator> {
        seq(b"!=").map(|_| Operator::NotEqual)
            | seq(b"?=").map(|_| Operator::AnyEqual)
            | seq(b"*=").map(|_| Operator::AllEqual)
            | seq(b"=").map(|_| Operator::Equal)
            | seq(b">=").map(|_| Operator::GreaterThanOrEqual)
            | seq(b">").map(|_| Operator::GreaterThan)
            | seq(b"<=").map(|_| Operator::LessThanOrEqual)
            | seq(b"<").map(|_| Operator::LessThan)
            | seq(b"!~").map(|_| Operator::NotLike)
            | seq(b"?~").map(|_| Operator::AnyLike)
            | seq(b"*~").map(|_| Operator::AllLike)
            | seq(b"~").map(|_| Operator::Like)
            | seq(b"NOTINSIDE").map(|_| Operator::NotInside)
            | seq(b"ALLINSIDE").map(|_| Operator::AllInside)
            | seq(b"ANYINSIDE").map(|_| Operator::AnyInside)
            | seq(b"NONEINSIDE").map(|_| Operator::NoneInside)
            | seq(b"INSIDE").map(|_| Operator::Inside)
            | seq(b"NOT IN").map(|_| Operator::NotIn)
            | seq(b"IN").map(|_| Operator::In)
            | seq(b"CONTAINSNOT").map(|_| Operator::ContainsNot)
            | seq(b"CONTAINSALL").map(|_| Operator::ContainsAll)
            | seq(b"CONTAINSANY").map(|_| Operator::ContainsAny)
            | seq(b"CONTAINSNONE").map(|_| Operator::ContainsNone)
            | seq(b"CONTAINS").map(|_| Operator::Contains)
    }

    pub fn string<'a>() -> Parser<'a, u8, String> {
        let string_pf = |quote_sym| {
            let special_char = sym(b'\\')
                | sym(b'/')
                | sym(b'"')
                | sym(b'\'')
                | sym(b'b').map(|_| b'\x08')
                | sym(b'f').map(|_| b'\x0C')
                | sym(b'n').map(|_| b'\n')
                | sym(b'r').map(|_| b'\r')
                | sym(b't').map(|_| b'\t');
            let escape_sequence = sym(b'\\') * special_char;
            let char_string = (none_of(b"\\\"") | escape_sequence)
                .repeat(1..)
                .convert(String::from_utf8);

            sym(quote_sym) * char_string.repeat(0..) - sym(quote_sym)
        };
        let string = string_pf(b'"') | string_pf(b'\'');
        string.map(|strings| strings.concat())
    }

    pub fn int<'a>() -> Parser<'a, u8, i64> {
        let number = sym(b'-').opt() + one_of(b"0123456789").repeat(1..);
        number
            .collect()
            .convert(std::str::from_utf8)
            .convert(i64::from_str)
    }

    pub fn set<'a>() -> Parser<'a, u8, Vec<Value>> {
        let elems = list(call(value), sym(b',') * space());
        sym(b'[') * space() * elems - sym(b']')
    }

    pub fn space<'a>() -> Parser<'a, u8, ()> {
        one_of(b" \t\r\n").repeat(0..).discard()
    }

    #[cfg(test)]
    mod tests {
        use super::super::Compile;
        use super::*;
        use pretty_assertions::assert_eq;
        use rstest::rstest;

        #[rstest]
        #[case(Ok(Operator::Equal), "=")]
        #[case(Ok(Operator::NotEqual), "!=")]
        #[case(Ok(Operator::AnyEqual), "?=")]
        #[case(Ok(Operator::AllEqual), "*=")]
        #[case(Ok(Operator::GreaterThan), ">")]
        #[case(Ok(Operator::GreaterThanOrEqual), ">=")]
        #[case(Ok(Operator::LessThan), "<")]
        #[case(Ok(Operator::LessThanOrEqual), "<=")]
        #[case(Ok(Operator::Like), "~")]
        #[case(Ok(Operator::NotLike), "!~")]
        #[case(Ok(Operator::AnyLike), "?~")]
        #[case(Ok(Operator::AllLike), "*~")]
        #[case(Ok(Operator::Inside), "INSIDE")]
        #[case(Ok(Operator::NotInside), "NOTINSIDE")]
        #[case(Ok(Operator::AllInside), "ALLINSIDE")]
        #[case(Ok(Operator::AnyInside), "ANYINSIDE")]
        #[case(Ok(Operator::NoneInside), "NONEINSIDE")]
        #[case(Ok(Operator::In), "IN")]
        #[case(Ok(Operator::NotIn), "NOT IN")]
        #[case(Ok(Operator::Contains), "CONTAINS")]
        #[case(Ok(Operator::ContainsNot), "CONTAINSNOT")]
        #[case(Ok(Operator::ContainsAll), "CONTAINSALL")]
        #[case(Ok(Operator::ContainsAny), "CONTAINSANY")]
        #[case(Ok(Operator::ContainsNone), "CONTAINSNONE")]
        #[case(Err(pom::Error::Mismatch { message: "seq [67, 79, 78, 84, 65, 73, 78, 83] expect: 67, found: 105".to_string(), position: 0 }), "invalid")]
        fn test_operator_parse_compile(
            #[case] expected: Result<Operator, pom::Error>,
            #[case] s: &str,
        ) {
            let parsed = operator().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(operator) = parsed {
                let compiled = operator.compile();
                assert_eq!(compiled, s);
            }
        }

        #[rstest]
        #[case(Ok(Field::Title), "title")]
        #[case(Ok(Field::Artists), "artist")]
        #[case(Ok(Field::Album), "album")]
        #[case(Ok(Field::AlbumArtists), "album_artist")]
        #[case(Ok(Field::Genre), "genre")]
        #[case(Ok(Field::Year), "year")]
        #[case(Err(pom::Error::Mismatch { message: "seq [121, 101, 97, 114] expect: 121, found: 105".to_string(), position: 0 }), "invalid")]
        fn test_field_parse_compile(#[case] expected: Result<Field, pom::Error>, #[case] s: &str) {
            let parsed = field().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(field) = parsed {
                let compiled = field.compile();
                assert_eq!(compiled, s);
            }
        }

        #[rstest]
        #[case(Ok(Value::String("foo".to_string())), "\"foo\"")]
        #[case(Ok(Value::Int(42)), "42")]
        #[case(Ok(Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)])), "[\"foo\", 42]")]
        #[case::nested(
            Ok(Value::Set(vec![
                Value::String("foo".to_string()),
                Value::Set(vec![Value::String("bar".to_string()), Value::Int(42)])
                ])),
                "[\"foo\", [\"bar\", 42]]"
            )]
        #[case(Ok(Value::Field(Field::Title)), "title")]
        #[case(Ok(Value::Field(Field::Artists)), "artist")]
        #[case(Ok(Value::Field(Field::Album)), "album")]
        #[case(Ok(Value::Field(Field::AlbumArtists)), "album_artist")]
        #[case(Ok(Value::Field(Field::Genre)), "genre")]
        #[case(Ok(Value::Field(Field::Year)), "year")]
        #[case(Err(pom::Error::Mismatch { message: "seq [121, 101, 97, 114] expect: 121, found: 34".to_string(), position: 0 }), "\"foo")]
        #[case(Err(pom::Error::Mismatch { message: "seq [121, 101, 97, 114] expect: 121, found: 91".to_string(), position: 0 }), "[foo, 42")]
        #[case(Err(pom::Error::Mismatch { message: "seq [121, 101, 97, 114] expect: 121, found: 105".to_string(), position: 0 }), "invalid")]
        fn test_value_parse_compile(#[case] expected: Result<Value, pom::Error>, #[case] s: &str) {
            let parsed = value().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(value) = parsed {
                let compiled = value.compile();
                assert_eq!(compiled, s);
            }
        }

        #[rstest]
        // we know that each part of a clause is parsed and compiled correctly, so we only need to test the combination
        #[case(Ok(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::String("foo".to_string())
        }), "title = \"foo\"")]
        #[case(Ok(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::Int(42)
        }), "title = 42")]
        #[case(Ok(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::Set(vec![Value::String("foo".to_string()), Value::Int(42)])
        }), "title = [\"foo\", 42]")]
        #[case(Ok(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::Equal,
            right: Value::Field(Field::Artists)
        }), "title = artist")]
        #[case(Err(pom::Error::Incomplete), "title")]
        #[case(Err(pom::Error::Mismatch { message: "seq [121, 101, 97, 114] expect: 121, found: 32".to_string(), position: 0 }), " = \"foo\"")]
        #[case(Err(pom::Error::Incomplete), "title = ")]
        #[case(Err(pom::Error::Mismatch { message: "seq [67, 79, 78, 84, 65, 73, 78, 83] expect: 67, found: 105".to_string(), position: 6 }), "title invalid \"foo\"")]
        // special cases
        #[case::left_has_spaces(Ok(LeafClause {
                left: Value::String("foo bar".to_string()),
                operator: Operator::Equal,
                right: Value::Int(42)
            }), "\"foo bar\" = 42")]
        #[case::operator_has_spaces(Ok(LeafClause {
            left: Value::Field(Field::Title),
            operator: Operator::NotIn,
            right: Value::String("foo bar".to_string())
        }), "title NOT IN \"foo bar\"")]
        fn test_leaf_clause_parse(
            #[case] expected: Result<LeafClause, pom::Error>,
            #[case] s: &str,
        ) {
            let parsed = leaf().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(clause) = parsed {
                let compiled = clause.compile();
                assert_eq!(compiled, s);
            }
        }

        #[rstest]
        // we know that each part of a clause is parsed and compiled correctly, so we only need to test the combination
        #[case(Ok(CompoundClause {
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
        }), "(title = \"foo\" AND artist = \"bar\")")]
        #[case(Ok(CompoundClause {
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
        }), "(title = \"foo\" OR artist = \"bar\")")]
        #[case(Err(pom::Error::Incomplete), "(title = \"foo\"")]
        fn test_compound_clause_parse(
            #[case] expected: Result<CompoundClause, pom::Error>,
            #[case] s: &str,
        ) {
            let parsed = compound().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(clause) = parsed {
                let compiled = clause.compile();
                assert_eq!(compiled, s);
            }
        }

        #[rstest]
        // we know that each part of a clause is parsed and compiled correctly, so we only need to test the combination
        #[case(Ok(Query {
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
        },), "((title = \"foo\" AND (artist = \"bar\" OR album = \"baz\")) AND year > 2020)")]
        fn test_query_parse(#[case] expected: Result<Query, pom::Error>, #[case] s: &str) {
            let parsed = query().parse(s.as_bytes());
            assert_eq!(parsed, expected);
            if let Ok(query) = parsed {
                let compiled = query.compile();
                assert_eq!(compiled, s);
            }
        }
    }
}
