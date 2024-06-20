use surrealdb::sql::{
    statements::{DeleteStatement, OutputStatement, RelateStatement, SelectStatement},
    Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Limit, Number, Operator, Order, Orders,
    Param, Part, Subquery, Table, Tables, Value, Values,
};

/// Query to add relations between two tables.
///
/// Compiles to:
///
/// ```sql, ignore
/// RELATE $source->rel->$target
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::relate;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: add a album to an artist
/// let statement = relate("id", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $id->artist_to_album->$album".into_query().unwrap()
/// );
///
/// // Example: add a album to multiple artists
/// let statement = relate("ids", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RELATE $ids->artist_to_album->$album".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn relate<Source: AsRef<str>, Target: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    target: Target,
    rel: Rel,
) -> RelateStatement {
    fn relate_statement(source: &str, target: &str, rel: &str) -> RelateStatement {
        RelateStatement {
            from: Value::Param(Param(Ident(source.into()))),
            kind: Value::Table(Table(rel.into())),
            with: Value::Param(Param(Ident(target.into()))),
            ..Default::default()
        }
    }

    relate_statement(source.as_ref(), target.as_ref(), rel.as_ref())
}

/// Query to unrelate two tables.
///
/// Compiles to:
/// ```sql, ignore
/// DELETE $source->rel WHERE out IN $target
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::unrelate;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: remove a album from an artist
/// let statement = unrelate("artist", "album", "artist_to_album");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "DELETE $artist->artist_to_album WHERE out IN $album".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn unrelate<Source: AsRef<str>, Target: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    target: Target,
    rel: Rel,
) -> DeleteStatement {
    fn unrelate_statement(source: &str, target: &str, rel: &str) -> DeleteStatement {
        DeleteStatement {
            what: Values(vec![Value::Idiom(Idiom(vec![
                Part::Start(Value::Param(Param(Ident(source.into())))),
                Part::Graph(Graph {
                    dir: Dir::Out,
                    what: Tables(vec![Table(rel.into())]),
                    expr: Fields::all(),
                    ..Default::default()
                }),
            ]))]),
            cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
                l: Value::Idiom(Idiom(vec![Part::Field(Ident("out".into()))])),
                o: Operator::Inside,
                r: Value::Param(Param(Ident(target.into()))),
            })))),
            ..Default::default()
        }
    }

    unrelate_statement(source.as_ref(), target.as_ref(), rel.as_ref())
}
/// Query to read items related to a source.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM $source->rel.out
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::read_related_out;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: read all the songs of an album
/// let statement = read_related_out("album", "album_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "SELECT * FROM $album->album_to_song.out".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_related_out<Source: AsRef<str>, Rel: AsRef<str>>(
    source: Source,
    rel: Rel,
) -> SelectStatement {
    fn read_related_statement(source: &str, rel: &str) -> SelectStatement {
        SelectStatement {
            expr: Fields::all(),
            what: Values(vec![Value::Idiom(Idiom(vec![
                Part::Start(Value::Param(Param(Ident(source.into())))),
                Part::Graph(Graph {
                    dir: Dir::Out,
                    what: Tables(vec![Table(rel.into())]),
                    expr: Fields::all(),
                    ..Default::default()
                }),
                Part::Field(Ident("out".into())),
            ]))]),
            ..Default::default()
        }
    }

    read_related_statement(source.as_ref(), rel.as_ref())
}

/// Query to read items related to a target
///
/// Compiles to:
///
/// ```sql, ignore
/// SELECT * FROM $target<-rel.in
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::read_related_in;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: read the artist of an album
/// let statement = read_related_in("album", "artist_to_album");
/// assert_eq!(
///    statement.into_query().unwrap(),
///   "SELECT * FROM $album<-artist_to_album.in".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn read_related_in<Target: AsRef<str>, Rel: AsRef<str>>(
    target: Target,
    rel: Rel,
) -> SelectStatement {
    fn read_related_statement(target: &str, rel: &str) -> SelectStatement {
        SelectStatement {
            expr: Fields::all(),
            what: Values(vec![Value::Idiom(Idiom(vec![
                Part::Start(Value::Param(Param(Ident(target.into())))),
                Part::Graph(Graph {
                    dir: Dir::In,
                    what: Tables(vec![Table(rel.into())]),
                    expr: Fields::all(),
                    ..Default::default()
                }),
                Part::Field(Ident("in".into())),
            ]))]),
            ..Default::default()
        }
    }

    read_related_statement(target.as_ref(), rel.as_ref())
}

/// Query to count the number of items in a table.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::len((SELECT * FROM table))
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of songs in the database
/// let statement = count("song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RETURN array::len((SELECT * FROM song))".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count<Table: AsRef<str>>(table: Table) -> OutputStatement {
    fn count_statement(table: &str) -> OutputStatement {
        OutputStatement {
            what: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                "array::len".into(),
                vec![Value::Subquery(Box::new(Subquery::Select(
                    SelectStatement {
                        expr: Fields::all(),
                        what: Values(vec![Value::Table(Table(table.into()))]),
                        ..Default::default()
                    },
                )))],
            ))),
            ..Default::default()
        }
    }

    count_statement(table.as_ref())
}

/// Query to count the number of items in a table that are not included in a relation.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::len((SELECT * FROM table WHERE count(->rel) = 0))
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count_orphaned;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of orphaned albums in the database
/// let statement = count_orphaned("album", "album_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RETURN array::len((SELECT * FROM album WHERE count(->album_to_song) = 0))".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count_orphaned<Table: AsRef<str>, Rel: AsRef<str>>(
    table: Table,
    rel: Rel,
) -> OutputStatement {
    fn count_orphaned_statement(table: &str, rel: &str) -> OutputStatement {
        OutputStatement {
            what: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                "array::len".into(),
                vec![Value::Subquery(Box::new(Subquery::Select(
                    SelectStatement {
                        expr: Fields::all(),
                        what: Values(vec![Value::Table(Table(table.into()))]),
                        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
                            l: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                                "count".into(),
                                vec![Value::Idiom(Idiom(vec![Part::Graph(Graph {
                                    dir: Dir::Out,
                                    what: Tables(vec![Table(rel.into())]),
                                    expr: Fields::all(),
                                    ..Default::default()
                                })]))],
                            ))),
                            o: Operator::Equal,
                            r: Value::Number(Number::Int(0)),
                        })))),
                        ..Default::default()
                    },
                )))],
            ))),
            ..Default::default()
        }
    }

    count_orphaned_statement(table.as_ref(), rel.as_ref())
}

/// Query to count the number of items in a table that are not included in both of the provided relations.
/// This is useful for counting orphaned items that are not included in either of the provided relations.
///
/// Compiles to:
/// ```sql, ignore
/// RETURN array::len((SELECT * FROM table WHERE count(->rel1.out) = 0 AND count(->rel2.out) = 0))
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::count_orphaned_both;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: count the number of orphaned artists in the database
/// let statement = count_orphaned_both("artist", "artist_to_album", "artist_to_song");
/// assert_eq!(
///     statement.into_query().unwrap(),
///     "RETURN array::len((SELECT * FROM artist WHERE count(->artist_to_album) = 0 AND count(->artist_to_song) = 0))".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn count_orphaned_both<Table: AsRef<str>, Rel1: AsRef<str>, Rel2: AsRef<str>>(
    table: Table,
    rel1: Rel1,
    rel2: Rel2,
) -> OutputStatement {
    fn count_orphaned_both_statement(table: &str, rel1: &str, rel2: &str) -> OutputStatement {
        OutputStatement {
            what: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                "array::len".into(),
                vec![Value::Subquery(Box::new(Subquery::Select(
                    SelectStatement {
                        expr: Fields::all(),
                        what: Values(vec![Value::Table(Table(table.into()))]),
                        cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
                            l: Value::Expression(Box::new(Expression::Binary {
                                l: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                                    "count".into(),
                                    vec![Value::Idiom(Idiom(vec![Part::Graph(Graph {
                                        dir: Dir::Out,
                                        what: Tables(vec![Table(rel1.into())]),
                                        expr: Fields::all(),
                                        ..Default::default()
                                    })]))],
                                ))),
                                o: Operator::Equal,
                                r: Value::Number(Number::Int(0)),
                            })),
                            o: Operator::And,
                            r: Value::Expression(Box::new(Expression::Binary {
                                l: Value::Function(Box::new(surrealdb::sql::Function::Normal(
                                    "count".into(),
                                    vec![Value::Idiom(Idiom(vec![Part::Graph(Graph {
                                        dir: Dir::Out,
                                        what: Tables(vec![Table(rel2.into())]),
                                        expr: Fields::all(),
                                        ..Default::default()
                                    })]))],
                                ))),
                                o: Operator::Equal,
                                r: Value::Number(Number::Int(0)),
                            })),
                        })))),
                        ..Default::default()
                    },
                )))],
            ))),
            ..Default::default()
        }
    }

    count_orphaned_both_statement(table.as_ref(), rel1.as_ref(), rel2.as_ref())
}

/// Query to run a full text search on a given field of a given table.
///
/// Compiles to:
/// ```sql, ignore
/// SELECT * FROM table WHERE field @@ $field ORDER BY relevance DESC LIMIT limit
/// ```
///
/// # Example
///
/// ```ignore
/// # use pretty_assertions::assert_eq;
/// use mecomp_storage::db::crud::queries::generic::full_text_search;
/// use surrealdb::opt::IntoQuery;
///
/// // Example: search for songs with the word "hello" in the title
/// let statement = full_text_search("song", "title", 10);
/// assert_eq!(
///    statement.into_query().unwrap(),
///   "SELECT * FROM song WHERE title @@ $title ORDER BY relevance DESC LIMIT 10".into_query().unwrap()
/// );
/// ```
#[must_use]
pub fn full_text_search<Table: AsRef<str>, Field: AsRef<str>>(
    table: Table,
    field: Field,
    limit: i64,
) -> SelectStatement {
    fn full_text_search_statement(table: &str, field: &str, limit: i64) -> SelectStatement {
        SelectStatement {
            expr: Fields::all(),
            what: Values(vec![Value::Table(Table(table.into()))]),
            cond: Some(Cond(Value::Expression(Box::new(Expression::Binary {
                l: Value::Idiom(Idiom(vec![Part::Field(Ident(field.into()))])),
                o: Operator::Matches(None),
                r: Value::Param(Param(Ident(field.into()))),
            })))),
            order: Some(Orders(vec![Order {
                order: Idiom(vec![Part::Field(Ident("relevance".into()))]),
                random: false,
                collate: false,
                numeric: false,
                direction: false,
            }])),
            limit: Some(Limit(Value::Number(limit.into()))),
            ..Default::default()
        }
    }

    full_text_search_statement(table.as_ref(), field.as_ref(), limit)
}

#[cfg(test)]
mod query_validation_tests {
    use pretty_assertions::assert_eq;
    use surrealdb::opt::IntoQuery;

    use super::*;

    #[test]
    fn test_relate() {
        let statement = relate("id", "album", "artist_to_album");
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $id->artist_to_album->$album".into_query().unwrap()
        );

        let statement = relate("ids", "album", "artist_to_album");
        assert_eq!(
            statement.into_query().unwrap(),
            "RELATE $ids->artist_to_album->$album".into_query().unwrap()
        );
    }

    #[test]
    fn test_unrelate() {
        let statement = unrelate("artist", "album", "artist_to_album");
        assert_eq!(
            statement.into_query().unwrap(),
            "DELETE $artist->artist_to_album WHERE out IN $album"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_related_out() {
        let statement = read_related_out("album", "album_to_song");
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $album->album_to_song.out"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_read_related_in() {
        let statement = read_related_in("album", "artist_to_album");
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM $album<-artist_to_album.in"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_count() {
        let statement = count("song");
        assert_eq!(
            statement.into_query().unwrap(),
            "RETURN array::len((SELECT * FROM song))"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_count_orphaned() {
        let statement = count_orphaned("album", "album_to_song");
        assert_eq!(
            statement.into_query().unwrap(),
            "RETURN array::len((SELECT * FROM album WHERE count(->album_to_song) = 0))"
                .into_query()
                .unwrap()
        );
    }

    #[test]
    fn test_count_orphaned_both() {
        let statement = count_orphaned_both("artist", "artist_to_album", "artist_to_song");
        assert_eq!(
            statement.into_query().unwrap(),
            "RETURN array::len((SELECT * FROM artist WHERE count(->artist_to_album) = 0 AND count(->artist_to_song) = 0))".into_query().unwrap()
        );
    }

    #[test]
    fn test_full_text_search() {
        let statement = full_text_search("song", "title", 10);
        assert_eq!(
            statement.into_query().unwrap(),
            "SELECT * FROM song WHERE title @@ $title ORDER BY relevance DESC LIMIT 10"
                .into_query()
                .unwrap()
        );
    }
}
