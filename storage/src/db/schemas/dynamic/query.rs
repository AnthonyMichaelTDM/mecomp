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
    clauses: Vec<Clause>,
    kind: CompoundKind,
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
