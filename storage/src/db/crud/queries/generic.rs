use surrealdb::sql::{
    statements::{DeleteStatement, RelateStatement, SelectStatement},
    Cond, Dir, Expression, Fields, Graph, Ident, Idiom, Operator, Param, Part, Table, Tables,
    Value, Values,
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
/// ```
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
/// ```
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
/// ```
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
/// ```
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
